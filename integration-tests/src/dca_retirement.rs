//! Legacy DCA state is planted through the migration's own storage aliases: the pallet crate is gone.

use crate::driver::HydrationTestDriver;
use crate::ice::PATH_TO_SNAPSHOT;
use crate::polkadot_test_net::{hydradx_run_to_next_block, Hydra, TestNet, ALICE, BOB, CHARLIE, DOT, HDX, LRNA, UNITS};
use amm_simulator::HydrationSimulator;
use codec::Decode;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::Weight;
use frame_support::{assert_ok, storage_alias, Blake2_128Concat, Twox64Concat};
use hydradx_runtime::migrations::dca::{
	LegacyOrder, LegacySchedule, MigrateDcaToIntents, RemainingAmounts, ScheduleIdsPerBlock, Schedules, DCA_RESERVE_ID,
	MAX_DCA_KEYS, MAX_SCHEDULES,
};
use hydradx_runtime::{AssetRegistry, Currencies, Runtime, RuntimeOrigin, System, Tokens, Treasury};
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use hydradx_traits::registry::Inspect;
use hydradx_traits::router::{PoolType, Trade};
use ice_solver::v4::Solver as IceSolver;
use ice_support::{DcaData, IntentData, IntentId, Solution};
use orml_traits::{MultiCurrency, MultiReservableCurrency, NamedMultiReservableCurrency};
use pallet_transaction_pause::{BoundedName, PausedTransactions};
use primitives::{AccountId, AssetId, Balance};
use sp_runtime::{Permill, StateVersion};
use xcm_emulator::{Network, TestExt};

const INTENT_RESERVE_ID: [u8; 8] = *b"ICE_int#";
const START_BLOCK: u32 = 10_000;
const PERIOD: u32 = 600;
const ENDOWMENT: Balance = 1_000 * UNITS;
const SLIPPAGE: Permill = Permill::from_percent(3);
const HOLLAR: AssetId = 222;
const ONE_HOLLAR: Balance = 1_000_000_000_000_000_000;
const BNC: AssetId = 14;
/// HOLLAR held by `ReserveAccount` in the `mainnet_apr` snapshot.
const SNAPSHOT_CUSTODY_HOLLAR: Balance = 27_967_822_764_337_836_667_721;

#[storage_alias(verbatim)]
type ScheduleOwnership = StorageDoubleMap<DCA, Blake2_128Concat, AccountId, Twox64Concat, u32, ()>;

#[storage_alias(verbatim)]
type ScheduleExecutionBlock = StorageMap<DCA, Blake2_128Concat, u32, u32>;

#[storage_alias(verbatim)]
type ScheduleIdSequencer = StorageValue<DCA, u32>;

type CombinedSimulatorState =
	<<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::State;
type Solver = IceSolver<HydrationSimulator<hydradx_runtime::HydrationSimulatorConfig>>;

fn sell(owner: AccountId, asset_in: AssetId, asset_out: AssetId, amount_in: Balance, total: Balance) -> LegacySchedule {
	LegacySchedule {
		owner,
		period: PERIOD,
		total_amount: total,
		max_retries: None,
		stability_threshold: None,
		slippage: Some(SLIPPAGE),
		order: LegacyOrder::Sell {
			asset_in,
			asset_out,
			amount_in,
			min_amount_out: 0,
			route: vec![],
		},
	}
}

/// `remaining: None` plants an orphan: a bare `Schedules` entry.
fn plant(id: u32, schedule: LegacySchedule, remaining: Option<Balance>, planned: Option<u32>) {
	if let Some(remaining) = remaining {
		RemainingAmounts::insert(id, remaining);
		ScheduleOwnership::insert(&schedule.owner, id, ());
	}
	if let Some(block) = planned {
		ScheduleIdsPerBlock::mutate(block, |ids| ids.get_or_insert_with(Vec::new).push(id));
		ScheduleExecutionBlock::insert(id, block);
	}
	ScheduleIdSequencer::put(id + 1);
	Schedules::insert(id, schedule);
}

/// Holds nothing at genesis, so balances can be asserted exactly.
fn owner() -> AccountId {
	AccountId::from([0xa1; 32])
}

fn reserve_legacy(owner: &AccountId, asset: AssetId, amount: Balance) {
	assert_ok!(Currencies::reserve_named(&DCA_RESERVE_ID, asset, owner, amount));
}

fn endow(owner: &AccountId, asset: AssetId, amount: Balance) {
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		owner.clone(),
		asset,
		amount as i128,
	));
}

fn reserved(id: &[u8; 8], asset: AssetId, owner: &AccountId) -> Balance {
	Currencies::reserved_balance_named(id, asset, owner)
}

fn ed(asset: AssetId) -> Balance {
	<AssetRegistry as Inspect>::existential_deposit(asset).expect("asset is registered")
}

fn migrate_unchecked() {
	MigrateDcaToIntents::on_runtime_upgrade();
}

/// Also asserts what `post_upgrade` checks, which only runs under try-runtime.
fn migrate() {
	migrate_unchecked();

	let prefix = dca_prefix();
	assert_eq!(
		sp_io::storage::next_key(&prefix).filter(|key| key.starts_with(&prefix)),
		None
	);
	let native: Vec<_> = pallet_balances::Reserves::<Runtime>::iter()
		.filter(|(_, reserves)| reserves.iter().any(|r| r.id == DCA_RESERVE_ID && r.amount > 0))
		.map(|(owner, _)| owner)
		.collect();
	assert_eq!(native, vec![]);
	let tokens: Vec<_> = orml_tokens::Reserves::<Runtime>::iter()
		.filter(|(_, _, reserves)| reserves.iter().any(|r| r.id == DCA_RESERVE_ID && r.amount > 0))
		.map(|(owner, asset, _)| (owner, asset))
		.collect();
	assert_eq!(tokens, vec![]);
	assert!(!PausedTransactions::<Runtime>::contains_key(pause_key(
		"DCA", "schedule"
	)));
}

fn pause_key(pallet: &str, call: &str) -> (BoundedName, BoundedName) {
	(
		BoundedName::truncate_from(pallet.as_bytes().to_vec()),
		BoundedName::truncate_from(call.as_bytes().to_vec()),
	)
}

fn dca_intents_of(owner: &AccountId) -> Vec<(IntentId, DcaData)> {
	let mut intents: Vec<_> = pallet_intent::AccountIntents::<Runtime>::iter_prefix(owner)
		.filter_map(|(id, ())| match pallet_intent::Intents::<Runtime>::get(id)?.data {
			IntentData::Dca(dca) => Some((id, dca)),
			_ => None,
		})
		.collect();
	intents.sort_by_key(|(id, _)| *id);
	intents
}

fn offered(id: IntentId) -> bool {
	pallet_intent::Pallet::<Runtime>::get_valid_intents()
		.iter()
		.any(|(offered, _)| *offered == id)
}

fn dca_prefix() -> Vec<u8> {
	sp_io::hashing::twox_128(b"DCA").to_vec()
}

fn genesis(f: impl FnOnce()) {
	TestNet::reset();
	Hydra::execute_with(|| {
		System::set_block_number(START_BLOCK);
		f();
	});
}

fn snapshot(f: impl FnOnce()) {
	TestNet::reset();
	HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		// The slim snapshot keeps `dcaorder` reserves of accounts it does not carry; nothing backs them.
		for owner in pallet_balances::Reserves::<Runtime>::iter_keys().collect::<Vec<_>>() {
			if Currencies::reserved_balance(HDX, &owner) == 0 {
				pallet_balances::Reserves::<Runtime>::mutate(&owner, |reserves| {
					reserves.retain(|r| r.id != DCA_RESERVE_ID)
				});
			}
		}
		for (owner, asset) in orml_tokens::Reserves::<Runtime>::iter_keys().collect::<Vec<_>>() {
			if orml_tokens::Accounts::<Runtime>::get(&owner, asset).reserved == 0 {
				orml_tokens::Reserves::<Runtime>::mutate(&owner, asset, |reserves| {
					reserves.retain(|r| r.id != DCA_RESERVE_ID)
				});
			}
		}
		f();
	});
}

fn run_solver_and_submit() -> Solution {
	let call = pallet_ice::Pallet::<Runtime>::run(
		System::block_number(),
		|intents: Vec<ice_support::Intent>, limits: Vec<(IntentId, Balance)>, state: CombinedSimulatorState| {
			Solver::solve_with_limits(
				intents,
				limits.into_iter().collect(),
				state,
				pallet_ice::ProtocolFee::<Runtime>::get(),
			)
			.ok()
		},
	)
	.expect("solver should produce a solution");
	let pallet_ice::Call::submit_solution { solution, .. } = call else {
		panic!("expected submit_solution");
	};
	hydradx_run_to_next_block();
	assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
		RuntimeOrigin::none(),
		solution.clone()
	));
	solution
}

#[test]
fn migration_should_convert_fixed_schedule_when_it_sells_an_orml_token() {
	genesis(|| {
		let alice = owner();
		endow(&alice, DOT, ENDOWMENT);
		reserve_legacy(&alice, DOT, 50 * UNITS);
		plant(
			1,
			sell(alice.clone(), DOT, HDX, 5 * UNITS, 100 * UNITS),
			Some(50 * UNITS),
			None,
		);

		migrate();

		let intents = dca_intents_of(&alice);
		assert_eq!(intents.len(), 1);
		assert_eq!(
			intents[0].1,
			DcaData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 5 * UNITS,
				amount_out: ed(HDX),
				slippage: SLIPPAGE,
				budget: Some(50 * UNITS),
				remaining_budget: 50 * UNITS,
				period: PERIOD,
				last_execution_block: START_BLOCK - PERIOD,
			}
		);
		assert_eq!(reserved(&DCA_RESERVE_ID, DOT, &alice), 0);
		assert_eq!(reserved(&INTENT_RESERVE_ID, DOT, &alice), 50 * UNITS);
		assert_eq!(Currencies::free_balance(DOT, &alice), ENDOWMENT - 50 * UNITS);
	});
}

#[test]
fn migration_should_reserve_twice_amount_in_and_free_the_excess_when_schedule_is_rolling() {
	genesis(|| {
		let alice = owner();
		let legacy_reserve = 10 * UNITS + 3 * UNITS / 10;
		endow(&alice, DOT, ENDOWMENT);
		reserve_legacy(&alice, DOT, legacy_reserve);
		plant(
			1,
			sell(alice.clone(), DOT, HDX, 5 * UNITS, 0),
			Some(legacy_reserve),
			None,
		);

		migrate();

		let intents = dca_intents_of(&alice);
		assert_eq!(intents.len(), 1);
		assert_eq!(intents[0].1.budget, None);
		assert_eq!(intents[0].1.remaining_budget, 10 * UNITS);
		assert_eq!(reserved(&DCA_RESERVE_ID, DOT, &alice), 0);
		assert_eq!(reserved(&INTENT_RESERVE_ID, DOT, &alice), 10 * UNITS);
		assert_eq!(Currencies::free_balance(DOT, &alice), ENDOWMENT - 10 * UNITS);
	});
}

#[test]
fn migration_should_convert_schedule_when_it_sells_native_hdx() {
	genesis(|| {
		let alice = owner();
		endow(&alice, HDX, ENDOWMENT);
		reserve_legacy(&alice, HDX, 50 * UNITS);
		plant(
			1,
			sell(alice.clone(), HDX, DOT, 5 * UNITS, 100 * UNITS),
			Some(50 * UNITS),
			None,
		);

		migrate();

		let intents = dca_intents_of(&alice);
		assert_eq!(intents.len(), 1);
		assert_eq!(intents[0].1.asset_in, HDX);
		assert_eq!(intents[0].1.amount_out, ed(DOT));
		assert_eq!(intents[0].1.remaining_budget, 50 * UNITS);
		assert_eq!(reserved(&DCA_RESERVE_ID, HDX, &alice), 0);
		assert_eq!(reserved(&INTENT_RESERVE_ID, HDX, &alice), 50 * UNITS);
		assert_eq!(Currencies::free_balance(HDX, &alice), ENDOWMENT - 50 * UNITS);
	});
}

#[test]
fn migration_should_delete_orphan_schedule_when_it_has_no_remaining_amount() {
	genesis(|| {
		let alice = owner();
		endow(&alice, DOT, ENDOWMENT);
		let intents_before = pallet_intent::Intents::<Runtime>::iter().count();
		plant(1, sell(alice.clone(), DOT, HDX, 5 * UNITS, 100 * UNITS), None, None);

		migrate();

		assert_eq!(Schedules::get(1), None);
		assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), intents_before);
		assert_eq!(Currencies::free_balance(DOT, &alice), ENDOWMENT);
		assert_eq!(Currencies::reserved_balance(DOT, &alice), 0);
	});
}

#[test]
fn migration_should_cancel_and_refund_when_fixed_budget_is_below_one_trade() {
	genesis(|| {
		let alice = owner();
		endow(&alice, DOT, ENDOWMENT);
		reserve_legacy(&alice, DOT, 4 * UNITS);
		plant(
			1,
			sell(alice.clone(), DOT, HDX, 5 * UNITS, 100 * UNITS),
			Some(4 * UNITS),
			None,
		);

		migrate();

		assert_eq!(dca_intents_of(&alice), vec![]);
		assert_eq!(reserved(&DCA_RESERVE_ID, DOT, &alice), 0);
		assert_eq!(Currencies::free_balance(DOT, &alice), ENDOWMENT);
	});
}

#[test]
fn migration_should_cancel_and_refund_when_order_is_buy() {
	genesis(|| {
		let alice = owner();
		endow(&alice, DOT, ENDOWMENT);
		reserve_legacy(&alice, DOT, 50 * UNITS);
		let schedule = LegacySchedule {
			order: LegacyOrder::Buy {
				asset_in: DOT,
				asset_out: HDX,
				amount_out: UNITS,
				max_amount_in: 5 * UNITS,
				route: vec![],
			},
			..sell(alice.clone(), DOT, HDX, 5 * UNITS, 100 * UNITS)
		};
		plant(1, schedule, Some(50 * UNITS), None);

		migrate();

		assert_eq!(dca_intents_of(&alice), vec![]);
		assert_eq!(reserved(&DCA_RESERVE_ID, DOT, &alice), 0);
		assert_eq!(Currencies::free_balance(DOT, &alice), ENDOWMENT);
	});
}

#[test]
fn migration_should_cancel_and_refund_when_intent_rejects_the_period() {
	genesis(|| {
		let alice = owner();
		endow(&alice, DOT, ENDOWMENT);
		reserve_legacy(&alice, DOT, 50 * UNITS);
		let schedule = LegacySchedule {
			period: 10,
			..sell(alice.clone(), DOT, HDX, 5 * UNITS, 100 * UNITS)
		};
		plant(1, schedule, Some(50 * UNITS), None);

		migrate();

		assert_eq!(dca_intents_of(&alice), vec![]);
		assert_eq!(reserved(&DCA_RESERVE_ID, DOT, &alice), 0);
		assert_eq!(reserved(&INTENT_RESERVE_ID, DOT, &alice), 0);
		assert_eq!(Currencies::free_balance(DOT, &alice), ENDOWMENT);
	});
}

#[test]
fn migration_should_release_whole_legacy_reserve_when_it_exceeds_remaining_amount() {
	genesis(|| {
		let alice = owner();
		endow(&alice, DOT, ENDOWMENT);
		reserve_legacy(&alice, DOT, 57 * UNITS);
		plant(
			1,
			sell(alice.clone(), DOT, HDX, 5 * UNITS, 100 * UNITS),
			Some(50 * UNITS),
			None,
		);

		migrate();

		assert_eq!(reserved(&DCA_RESERVE_ID, DOT, &alice), 0);
		assert_eq!(reserved(&INTENT_RESERVE_ID, DOT, &alice), 50 * UNITS);
		assert_eq!(Currencies::free_balance(DOT, &alice), ENDOWMENT - 50 * UNITS);
	});
}

#[test]
fn migration_should_leave_legacy_reserve_when_owner_has_no_schedule() {
	genesis(|| {
		let stranger = owner();
		endow(&stranger, DOT, ENDOWMENT);
		reserve_legacy(&stranger, DOT, 7 * UNITS);

		migrate_unchecked();

		assert_eq!(reserved(&DCA_RESERVE_ID, DOT, &stranger), 7 * UNITS);
	});
}

#[test]
fn migration_should_remove_only_dca_pause_entry_when_transactions_are_paused() {
	genesis(|| {
		PausedTransactions::<Runtime>::insert(pause_key("DCA", "schedule"), ());
		PausedTransactions::<Runtime>::insert(pause_key("Stableswap", "buy"), ());

		migrate();

		assert!(!PausedTransactions::<Runtime>::contains_key(pause_key(
			"DCA", "schedule"
		)));
		assert!(PausedTransactions::<Runtime>::contains_key(pause_key(
			"Stableswap",
			"buy"
		)));
	});
}

#[test]
fn migrated_intent_should_first_be_offered_at_legacy_planned_block_when_it_is_in_the_future() {
	genesis(|| {
		let alice = owner();
		let planned = START_BLOCK + 40;
		endow(&alice, DOT, ENDOWMENT);
		reserve_legacy(&alice, DOT, 50 * UNITS);
		let schedule = LegacySchedule {
			period: 30,
			..sell(alice.clone(), DOT, HDX, 5 * UNITS, 100 * UNITS)
		};
		plant(1, schedule, Some(50 * UNITS), Some(planned));

		migrate();

		let (id, dca) = dca_intents_of(&alice).remove(0);
		// Offered at P - 1, so it settles in P, where the schedule traded.
		assert_eq!(dca.last_execution_block, planned - 1 - 30);
		// Beyond the early window (min(150, period / 2) = 15 blocks) nothing can offer it.
		System::set_block_number(planned - 1 - 16);
		assert!(!offered(id));
		System::set_block_number(planned - 1);
		assert!(offered(id));
	});
}

#[test]
fn migrated_intent_should_be_offered_immediately_when_legacy_planned_block_is_in_the_past() {
	genesis(|| {
		let alice = owner();
		endow(&alice, DOT, ENDOWMENT);
		reserve_legacy(&alice, DOT, 50 * UNITS);
		plant(
			1,
			sell(alice.clone(), DOT, HDX, 5 * UNITS, 100 * UNITS),
			Some(50 * UNITS),
			Some(START_BLOCK - 5),
		);

		migrate();

		let (id, dca) = dca_intents_of(&alice).remove(0);
		assert_eq!(dca.last_execution_block, START_BLOCK - PERIOD);
		assert!(offered(id));
	});
}

#[test]
fn migrated_intent_should_be_offered_immediately_when_schedule_has_no_planned_block() {
	genesis(|| {
		let alice = owner();
		endow(&alice, DOT, ENDOWMENT);
		reserve_legacy(&alice, DOT, 50 * UNITS);
		plant(
			1,
			sell(alice.clone(), DOT, HDX, 5 * UNITS, 100 * UNITS),
			Some(50 * UNITS),
			None,
		);

		migrate();

		let (id, dca) = dca_intents_of(&alice).remove(0);
		assert_eq!(dca.last_execution_block, START_BLOCK - PERIOD);
		assert!(offered(id));
	});
}

#[test]
fn migration_should_convert_both_schedules_when_owner_has_two_in_the_same_asset() {
	genesis(|| {
		let alice = owner();
		endow(&alice, DOT, ENDOWMENT);
		reserve_legacy(&alice, DOT, 80 * UNITS);
		plant(
			1,
			sell(alice.clone(), DOT, HDX, 5 * UNITS, 100 * UNITS),
			Some(50 * UNITS),
			None,
		);
		plant(
			2,
			sell(alice.clone(), DOT, HDX, 3 * UNITS, 30 * UNITS),
			Some(30 * UNITS),
			None,
		);

		migrate();

		let budgets: Vec<_> = dca_intents_of(&alice)
			.into_iter()
			.map(|(_, dca)| (dca.amount_in, dca.remaining_budget))
			.collect();
		assert_eq!(budgets, vec![(5 * UNITS, 50 * UNITS), (3 * UNITS, 30 * UNITS)]);
		assert_eq!(reserved(&DCA_RESERVE_ID, DOT, &alice), 0);
		assert_eq!(reserved(&INTENT_RESERVE_ID, DOT, &alice), 80 * UNITS);
	});
}

#[test]
fn migration_should_leave_no_key_under_dca_prefix_when_complete() {
	genesis(|| {
		let alice = owner();
		let bob = AccountId::from([0xb0; 32]);
		endow(&alice, DOT, ENDOWMENT);
		reserve_legacy(&alice, DOT, 50 * UNITS);
		plant(
			1,
			sell(alice.clone(), DOT, HDX, 5 * UNITS, 100 * UNITS),
			Some(50 * UNITS),
			Some(START_BLOCK + 7),
		);
		plant(2, sell(bob, DOT, HDX, 5 * UNITS, 100 * UNITS), None, None);
		let prefix = dca_prefix();
		assert!(sp_io::storage::next_key(&prefix).is_some_and(|key| key.starts_with(&prefix)));

		migrate();

		assert_eq!(
			sp_io::storage::next_key(&prefix).filter(|key| key.starts_with(&prefix)),
			None
		);
	});
}

#[test]
fn migration_should_change_nothing_when_run_twice() {
	genesis(|| {
		let alice = owner();
		endow(&alice, DOT, ENDOWMENT);
		reserve_legacy(&alice, DOT, 50 * UNITS);
		plant(
			1,
			sell(alice.clone(), DOT, HDX, 5 * UNITS, 100 * UNITS),
			Some(50 * UNITS),
			None,
		);
		migrate();
		assert_eq!(dca_intents_of(&alice).len(), 1);
		let root = sp_io::storage::root(StateVersion::V1);

		migrate();

		assert_eq!(sp_io::storage::root(StateVersion::V1), root);
	});
}

fn numbered_owner(i: u32) -> AccountId {
	let mut raw = [0xa2; 32];
	raw[..4].copy_from_slice(&i.to_le_bytes());
	AccountId::from(raw)
}

fn plant_convertible(count: u32) -> Vec<AccountId> {
	(0..count)
		.map(|i| {
			let owner = numbered_owner(i);
			endow(&owner, DOT, ENDOWMENT);
			reserve_legacy(&owner, DOT, 50 * UNITS);
			plant(
				i + 1,
				sell(owner.clone(), DOT, HDX, 5 * UNITS, 100 * UNITS),
				Some(50 * UNITS),
				None,
			);
			owner
		})
		.collect()
}

fn assert_migration_changes_nothing() {
	let root = sp_io::storage::root(StateVersion::V1);

	migrate_unchecked();

	assert_eq!(sp_io::storage::root(StateVersion::V1), root);
}

#[test]
fn migration_should_convert_every_schedule_when_exactly_max_schedules_exist() {
	genesis(|| {
		let owners = plant_convertible(MAX_SCHEDULES as u32);

		migrate();

		let intents: usize = owners.iter().map(|owner| dca_intents_of(owner).len()).sum();
		assert_eq!(intents, MAX_SCHEDULES);
	});
}

#[test]
fn migration_should_change_nothing_when_more_than_max_schedules_exist() {
	genesis(|| {
		plant_convertible(MAX_SCHEDULES as u32 + 1);
		PausedTransactions::<Runtime>::insert(pause_key("DCA", "schedule"), ());

		assert_migration_changes_nothing();
	});
}

#[test]
fn migration_should_change_nothing_when_more_than_max_schedules_planned_blocks_exist() {
	genesis(|| {
		plant_convertible(1);
		for block in 0..=MAX_SCHEDULES as u32 {
			ScheduleIdsPerBlock::insert(START_BLOCK + block, vec![1]);
		}
		PausedTransactions::<Runtime>::insert(pause_key("DCA", "schedule"), ());

		assert_migration_changes_nothing();
	});
}

/// Raw mainnet values at block 17306764.
#[test]
fn legacy_aliases_should_decode_mainnet_values_when_read_at_their_keys() {
	const RAW: [(&str, &str); 10] = [
		("0x100f0c53fa332d4d9712c66fd92efcb6347d9208c3a12dab846866426727fa0ba76891f6c00e9fc6ffb2c09ef26e8c84960e0000", "0xcc542bfb6f31dccac106a6e5700ff1cf06a4c9f4c4740f538dcfb521285ab113928824000000f2052a01000000000000000000000105000130750000000500000000000000242932a90200000000000000000000000000000000000000000000000000000000"),
		("0x100f0c53fa332d4d9712c66fd92efcb6347d9208c3a12dab846866426727fa0ba3576662fc6df0485a78100d9f2fde8966660000", "0x58c21ff21fad655af9af8412a1dae595dd9250a40c4b8c5966dda68b36765054201c0000000070820dfb4669a81f0000000000000105000130750000001e00000016000000300cef59cc76287b60000000000000000000000000000000000000000000000000"),
		("0x100f0c53fa332d4d9712c66fd92efcb6347d9208c3a12dab846866426727fa0b20393ec6a46313d9e079b62877fa4f8b98750000", "0x6d6f646c70792f747273727900000000000000000000000000000000000000001e00000000000000000000000000000000000000010900011027000000010000000000000000a0d424f200000000000000000000000000000000000000000000000000000004030100000000000000"),
		("0x100f0c53fa332d4d9712c66fd92efcb6347d9208c3a12dab846866426727fa0bc4b38e1da411970b040d6fcae036c65ed6920000", "0x41ddf2ded434f3b236eca63124ea45b9034a03249dec7b072c2b4efa8efa3eaec0a800000000000000000000000000000000000001050001307500000056040000450000000000d8620897a0720300000000000000000000000000000000000000000000001404560400006e000000026e0000006e000000de00000006b80b0000de000000e903000002b2020000e9030000b202000004b202000045000000"),
		("0x100f0c53fa332d4d9712c66fd92efcb6ffc5d4b9530f9c21bd976ef7a9775515a76891f6c00e9fc6ffb2c09ef26e8c84960e0000", "0xd96680caf90000000000000000000000"),
		("0x100f0c53fa332d4d9712c66fd92efcb6ffc5d4b9530f9c21bd976ef7a977551520393ec6a46313d9e079b62877fa4f8b98750000", "0x3cccfd6be40100000000000000000000"),
		("0x100f0c53fa332d4d9712c66fd92efcb6ffc5d4b9530f9c21bd976ef7a9775515c4b38e1da411970b040d6fcae036c65ed6920000", "0x30da599bdc0334e90600000000000000"),
		("0x100f0c53fa332d4d9712c66fd92efcb6fbfce3131283f440b44ebe1a6e6d97f6351af555a4a0e61d43c4266594eecf611b162101", "0x04960e0000"),
		("0x100f0c53fa332d4d9712c66fd92efcb6fbfce3131283f440b44ebe1a6e6d97f6dcb949e68771ad3542e3dba035c5f797a6140801", "0x0498750000"),
		("0x100f0c53fa332d4d9712c66fd92efcb6fbfce3131283f440b44ebe1a6e6d97f61498dfafd9a7c7a74da7f325e29c1dba378d0801", "0x04d6920000"),
	];
	fn account(hex: &str) -> AccountId {
		AccountId::decode(&mut &hex::decode(hex).expect("valid hex")[..]).expect("32 bytes")
	}
	fn trade(pool: PoolType<AssetId>, asset_in: AssetId, asset_out: AssetId) -> Trade<AssetId> {
		Trade {
			pool,
			asset_in,
			asset_out,
		}
	}

	genesis(|| {
		for (key, value) in RAW {
			let decode = |h: &str| hex::decode(h.trim_start_matches("0x")).expect("valid hex");
			sp_io::storage::set(&decode(key), &decode(value));
		}

		assert_eq!(
			Schedules::get(3734),
			Some(LegacySchedule {
				owner: account("cc542bfb6f31dccac106a6e5700ff1cf06a4c9f4c4740f538dcfb521285ab113"),
				period: 2_394_258,
				total_amount: 1_280_000_000_000,
				max_retries: Some(5),
				stability_threshold: None,
				slippage: Some(Permill::from_parts(30_000)),
				order: LegacyOrder::Sell {
					asset_in: 5,
					asset_out: 0,
					amount_in: 11_428_571_428,
					min_amount_out: 0,
					route: vec![],
				},
			})
		);
		assert_eq!(
			Schedules::get(26214),
			Some(LegacySchedule {
				owner: account("58c21ff21fad655af9af8412a1dae595dd9250a40c4b8c5966dda68b36765054"),
				period: 7_200,
				total_amount: 149_500_000_000_000_000_000_000,
				max_retries: Some(5),
				stability_threshold: None,
				slippage: Some(Permill::from_parts(30_000)),
				order: LegacyOrder::Sell {
					asset_in: 30,
					asset_out: 22,
					amount_in: 1_779_761_904_761_904_761_904,
					min_amount_out: 0,
					route: vec![],
				},
			})
		);
		assert_eq!(
			Schedules::get(30104),
			Some(LegacySchedule {
				owner: hydradx_runtime::Treasury::account_id(),
				period: 30,
				total_amount: 0,
				max_retries: Some(9),
				stability_threshold: None,
				slippage: Some(Permill::from_parts(10_000)),
				order: LegacyOrder::Sell {
					asset_in: 1,
					asset_out: 0,
					amount_in: 1_040_000_000_000,
					min_amount_out: 0,
					route: vec![trade(PoolType::Omnipool, 1, 0)],
				},
			})
		);
		assert_eq!(
			Schedules::get(37590),
			Some(LegacySchedule {
				owner: account("41ddf2ded434f3b236eca63124ea45b9034a03249dec7b072c2b4efa8efa3eae"),
				period: 43_200,
				total_amount: 0,
				max_retries: Some(5),
				stability_threshold: None,
				slippage: Some(Permill::from_parts(30_000)),
				order: LegacyOrder::Sell {
					asset_in: 1110,
					asset_out: 69,
					amount_in: 63_600_000_000_000_000_000,
					min_amount_out: 0,
					route: vec![
						trade(PoolType::Aave, 1110, 110),
						trade(PoolType::Stableswap(110), 110, 222),
						trade(PoolType::UniswapV3(3000), 222, 1001),
						trade(PoolType::Stableswap(690), 1001, 690),
						trade(PoolType::Aave, 690, 69),
					],
				},
			})
		);
		assert_eq!(RemainingAmounts::get(3734), Some(1_072_844_269_273));
		assert_eq!(RemainingAmounts::get(30104), Some(2_080_575_966_268));
		assert_eq!(RemainingAmounts::get(37590), Some(127_484_524_797_917_518_384));
		assert_eq!(RemainingAmounts::get(26214), None);
		assert_eq!(ScheduleIdsPerBlock::get(18_945_563), Some(vec![3734]));
		assert_eq!(ScheduleIdsPerBlock::get(17_306_790), Some(vec![30104]));
		assert_eq!(ScheduleIdsPerBlock::get(17_337_655), Some(vec![37590]));
	});
}

#[test]
fn migration_should_convert_rolling_schedule_when_it_sells_a_bound_erc20() {
	snapshot(|| {
		let owner: AccountId = CHARLIE.into();
		assert!(<AssetRegistry as hydradx_traits::BoundErc20>::contract_address(HOLLAR).is_some());
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(Treasury::account_id()),
			owner.clone(),
			HOLLAR,
			50 * ONE_HOLLAR,
		));
		let legacy_reserve = 20 * ONE_HOLLAR + 4 * ONE_HOLLAR / 10;
		reserve_legacy(&owner, HOLLAR, legacy_reserve);
		plant(
			u32::MAX - 1,
			sell(owner.clone(), HOLLAR, HDX, 10 * ONE_HOLLAR, 0),
			Some(legacy_reserve),
			None,
		);

		migrate();

		let intents = dca_intents_of(&owner);
		assert_eq!(intents.len(), 1);
		assert_eq!(intents[0].1.remaining_budget, 20 * ONE_HOLLAR);
		assert_eq!(reserved(&DCA_RESERVE_ID, HOLLAR, &owner), 0);
		assert_eq!(reserved(&INTENT_RESERVE_ID, HOLLAR, &owner), 20 * ONE_HOLLAR);
		assert_eq!(Currencies::free_balance(HOLLAR, &owner), 30 * ONE_HOLLAR);
	});
}

fn custody() -> AccountId {
	<Runtime as pallet_currencies::Config>::ReserveAccount::get()
}

/// Grows the `dcaorder` receipt past what custody holds, so the erc20 transfer back reverts.
fn reserve_hollar_beyond_custody(owner: &AccountId) {
	assert_eq!(Currencies::free_balance(HOLLAR, &custody()), SNAPSHOT_CUSTODY_HOLLAR);
	assert_ok!(Currencies::transfer(
		RuntimeOrigin::signed(Treasury::account_id()),
		owner.clone(),
		HOLLAR,
		50 * ONE_HOLLAR,
	));
	reserve_legacy(owner, HOLLAR, 20 * ONE_HOLLAR);
	let excess = SNAPSHOT_CUSTODY_HOLLAR + ONE_HOLLAR;
	assert_ok!(<Tokens as MultiCurrency<_>>::deposit(HOLLAR, owner, excess));
	assert_ok!(<Tokens as NamedMultiReservableCurrency<_>>::reserve_named(
		&DCA_RESERVE_ID,
		HOLLAR,
		owner,
		excess
	));
	plant(
		u32::MAX - 2,
		sell(owner.clone(), HOLLAR, HDX, 10 * ONE_HOLLAR, 0),
		None,
		None,
	);
}

fn plant_healthy_hdx_schedule(owner: &AccountId) {
	endow(owner, HDX, ENDOWMENT);
	reserve_legacy(owner, HDX, 50 * UNITS);
	plant(
		u32::MAX - 1,
		sell(owner.clone(), HDX, BNC, 5 * UNITS, 100 * UNITS),
		Some(50 * UNITS),
		None,
	);
}

#[test]
fn migration_should_keep_legacy_reserve_record_when_bound_erc20_release_fails() {
	snapshot(|| {
		let owner: AccountId = CHARLIE.into();
		reserve_hollar_beyond_custody(&owner);

		migrate_unchecked();

		assert_eq!(
			reserved(&DCA_RESERVE_ID, HOLLAR, &owner),
			SNAPSHOT_CUSTODY_HOLLAR + 21 * ONE_HOLLAR
		);
	});
}

#[test]
fn migration_should_keep_tokens_in_reserve_account_custody_when_bound_erc20_release_fails() {
	snapshot(|| {
		let owner: AccountId = CHARLIE.into();
		reserve_hollar_beyond_custody(&owner);

		migrate_unchecked();

		assert_eq!(
			Currencies::free_balance(HOLLAR, &custody()),
			SNAPSHOT_CUSTODY_HOLLAR + 20 * ONE_HOLLAR
		);
		assert_eq!(Currencies::free_balance(HOLLAR, &owner), 30 * ONE_HOLLAR);
	});
}

#[test]
fn migration_should_convert_schedule_of_another_owner_when_bound_erc20_release_fails() {
	snapshot(|| {
		let alice: AccountId = ALICE.into();
		plant_healthy_hdx_schedule(&alice);
		reserve_hollar_beyond_custody(&CHARLIE.into());

		migrate_unchecked();

		let intents = dca_intents_of(&alice);
		assert_eq!(intents.len(), 1);
		assert_eq!(intents[0].1.remaining_budget, 50 * UNITS);
		assert_eq!(reserved(&DCA_RESERVE_ID, HDX, &alice), 0);
		assert_eq!(reserved(&INTENT_RESERVE_ID, HDX, &alice), 50 * UNITS);
	});
}

#[test]
fn migration_should_keep_dca_pause_entry_when_bound_erc20_release_fails() {
	snapshot(|| {
		plant_healthy_hdx_schedule(&ALICE.into());
		reserve_hollar_beyond_custody(&CHARLIE.into());
		PausedTransactions::<Runtime>::insert(pause_key("DCA", "schedule"), ());

		migrate_unchecked();

		assert!(PausedTransactions::<Runtime>::contains_key(pause_key(
			"DCA", "schedule"
		)));
	});
}

#[test]
fn migration_should_keep_keys_under_dca_prefix_when_bound_erc20_release_fails() {
	snapshot(|| {
		plant_healthy_hdx_schedule(&ALICE.into());
		reserve_hollar_beyond_custody(&CHARLIE.into());

		migrate_unchecked();

		// Both `Schedules`, `ScheduleOwnership` and `ScheduleIdSequencer`.
		assert_eq!(dca_key_count(), 4);
	});
}

#[test]
fn migration_should_release_legacy_reserve_when_run_again_after_bound_erc20_release_failed() {
	snapshot(|| {
		let owner: AccountId = CHARLIE.into();
		reserve_hollar_beyond_custody(&owner);
		migrate_unchecked();
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(Treasury::account_id()),
			custody(),
			HOLLAR,
			ONE_HOLLAR,
		));

		migrate();

		assert_eq!(reserved(&DCA_RESERVE_ID, HOLLAR, &owner), 0);
		assert_eq!(
			Currencies::free_balance(HOLLAR, &owner),
			SNAPSHOT_CUSTODY_HOLLAR + 51 * ONE_HOLLAR
		);
		assert_eq!(Currencies::free_balance(HOLLAR, &custody()), 0);
	});
}

#[test]
fn migration_should_not_convert_schedule_again_when_run_again_after_unfinished_run() {
	snapshot(|| {
		let alice: AccountId = ALICE.into();
		plant_healthy_hdx_schedule(&alice);
		reserve_hollar_beyond_custody(&CHARLIE.into());
		migrate_unchecked();

		migrate_unchecked();

		assert_eq!(dca_intents_of(&alice).len(), 1);
	});
}

fn dca_key_count() -> usize {
	let prefix = dca_prefix();
	let mut at = prefix.clone();
	let mut count = 0;
	while let Some(next) = sp_io::storage::next_key(&at).filter(|key| key.starts_with(&prefix)) {
		count += 1;
		at = next;
	}
	count
}

#[test]
fn migration_should_change_nothing_when_more_than_max_dca_keys_exist() {
	genesis(|| {
		plant_convertible(1);
		for id in 0..=MAX_DCA_KEYS {
			ScheduleExecutionBlock::insert(id, START_BLOCK);
		}
		PausedTransactions::<Runtime>::insert(pause_key("DCA", "schedule"), ());

		assert_migration_changes_nothing();
	});
}

#[test]
fn migration_should_charge_proof_size_per_read_when_retiring_an_orphan_schedule() {
	genesis(|| {
		plant(1, sell(owner(), DOT, HDX, 5 * UNITS, 100 * UNITS), None, None);

		let weight = MigrateDcaToIntents::on_runtime_upgrade();

		// Five reads (schedule, two DCA keys, remaining amount, reserve) and two writes; `clear_prefix` counts
		// only backend keys, and these sit in the overlay.
		assert_eq!(weight, Weight::from_parts(325_000_000, 2_560));
	});
}

/// Like mainnet's indebted aToken sellers: the transfer runs Aave's health-factor check, over 400k gas.
#[test]
fn migration_should_convert_atoken_schedule_when_owner_borrows_against_many_collateral_reserves() {
	use crate::liquidation::{borrow, supply};
	use amm_simulator::aave::Simulator as AaveSimulator;
	use hydradx_runtime::evm::precompiles::erc20_mapping::HydraErc20Mapping;
	use hydradx_runtime::evm::Executor;
	use hydradx_runtime::{ice_simulator_provider, EVMAccounts};
	use hydradx_traits::amm::AmmSimulator;
	use hydradx_traits::evm::{CallContext, Erc20Encoding, InspectEvmAccounts, EVM};
	use hydradx_traits::BoundErc20;
	use sp_core::{H256, U256};

	const ADOT: AssetId = 1001;
	const SNAPSHOT_DOT: AssetId = 5;

	snapshot(|| {
		let owner = AccountId::from([0xdc; 32]);
		let evm = EVMAccounts::evm_address(&owner);
		let pool = pallet_liquidation::BorrowingContract::<Runtime>::get();
		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool));
		endow(&owner, HDX, 1_000 * UNITS);
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(owner.clone())));

		let pairs = AaveSimulator::<ice_simulator_provider::Aave<Runtime>>::snapshot().pairs;
		let mut collateral = 0;
		for (underlying, atoken) in pairs {
			if AssetRegistry::contract_address(underlying).is_some()
				|| AssetRegistry::contract_address(atoken).is_none()
			{
				continue;
			}
			let unit = 10u128.pow(ed(underlying).checked_ilog10().unwrap_or(6) + 2);
			let supplied = frame_support::storage::with_transaction(|| {
				let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
					endow(&owner, underlying, 20_000 * unit);
					supply(
						pool,
						evm,
						HydraErc20Mapping::encode_evm_address(underlying),
						10_000 * unit,
					);
				}));
				match outcome {
					Ok(()) => sp_runtime::TransactionOutcome::Commit(Ok::<_, sp_runtime::DispatchError>(true)),
					Err(_) => sp_runtime::TransactionOutcome::Rollback(Ok(false)),
				}
			})
			.unwrap_or(false);
			collateral += u32::from(supplied);
		}
		assert!(
			collateral >= 6,
			"only {collateral} collateral reserves could be supplied"
		);

		let legacy_reserve = Currencies::free_balance(ADOT, &owner) / 4;
		reserve_legacy(&owner, ADOT, legacy_reserve);
		borrow(
			pool,
			evm,
			HydraErc20Mapping::encode_evm_address(SNAPSHOT_DOT),
			100 * 10u128.pow(10),
		);

		// Guard: without extra gas this owner cannot move aDOT into custody at all.
		let contract = AssetRegistry::contract_address(ADOT).expect("aDOT is bound");
		let custody = EVMAccounts::evm_address(&<Runtime as pallet_currencies::Config>::ReserveAccount::get());
		let mut data = sp_io::hashing::keccak_256(b"transfer(address,uint256)")[..4].to_vec();
		data.extend_from_slice(H256::from(custody).as_bytes());
		data.extend_from_slice(&U256::from(legacy_reserve).to_big_endian());
		let at_base_limit = frame_support::storage::with_transaction(|| {
			let call = Executor::<Runtime>::call(CallContext::new_call(contract, evm), data, U256::zero(), 400_000);
			sp_runtime::TransactionOutcome::Rollback(Ok::<_, sp_runtime::DispatchError>(call.exit_reason))
		})
		.expect("rollback");
		assert!(!matches!(at_base_limit, pallet_evm::ExitReason::Succeed(_)));

		plant(
			u32::MAX - 1,
			sell(owner.clone(), ADOT, HDX, legacy_reserve / 4, legacy_reserve),
			Some(legacy_reserve),
			None,
		);

		migrate();

		let intents = dca_intents_of(&owner);
		assert_eq!(intents.len(), 1);
		assert_eq!(intents[0].1.remaining_budget, legacy_reserve);
		assert_eq!(reserved(&DCA_RESERVE_ID, ADOT, &owner), 0);
		assert_eq!(reserved(&INTENT_RESERVE_ID, ADOT, &owner), legacy_reserve);
	});
}

#[test]
fn migrated_intent_should_settle_through_ice_when_its_period_has_elapsed() {
	snapshot(|| {
		let alice: AccountId = ALICE.into();
		assert_ok!(hydradx_runtime::Omnipool::set_slip_fee(
			RuntimeOrigin::root(),
			Some(pallet_omnipool::types::SlipFeeConfig {
				max_slip_fee: Permill::from_percent(5),
			})
		));
		endow(&alice, HDX, 10_000 * UNITS);
		reserve_legacy(&alice, HDX, 4_000 * UNITS);
		plant(
			u32::MAX - 1,
			LegacySchedule {
				period: 15,
				slippage: Some(Permill::from_percent(10)),
				..sell(alice.clone(), HDX, BNC, 200 * UNITS, 4_000 * UNITS)
			},
			Some(4_000 * UNITS),
			None,
		);

		migrate();
		let (id, _) = dca_intents_of(&alice).remove(0);
		let bnc_before = Currencies::total_balance(BNC, &alice);

		let solution = run_solver_and_submit();

		assert!(solution.resolved_intents.iter().any(|resolved| resolved.id == id));
		assert_eq!(Currencies::total_balance(BNC, &alice) - bnc_before, 13_487_774_062_934);
		assert_eq!(dca_intents_of(&alice)[0].1.remaining_budget, 3_800 * UNITS);
	});
}

#[test]
fn migrated_hub_asset_intent_should_settle_through_ice_when_schedule_sells_h2o() {
	snapshot(|| {
		let bob: AccountId = BOB.into();
		let amount_in = 1_040_000_000_000;
		endow(&bob, LRNA, 100 * UNITS);
		reserve_legacy(&bob, LRNA, 2 * amount_in);
		plant(
			u32::MAX - 1,
			LegacySchedule {
				period: 30,
				slippage: Some(Permill::from_percent(1)),
				..sell(bob.clone(), LRNA, HDX, amount_in, 0)
			},
			Some(2 * amount_in),
			None,
		);

		migrate();
		let (id, _) = dca_intents_of(&bob).remove(0);
		let hdx_before = Currencies::total_balance(HDX, &bob);

		let solution = run_solver_and_submit();

		assert!(solution.resolved_intents.iter().any(|resolved| resolved.id == id));
		assert_eq!(Currencies::total_balance(HDX, &bob) - hdx_before, 3_700_798_942_397_350);
	});
}
