#![cfg(test)]
use frame_support::{
	assert_ok,
	dispatch::{Dispatchable, GetCallMetadata},
	sp_runtime::{
		traits::{AccountIdConversion, Block as BlockT},
		FixedU128, Permill,
	},
	traits::GenesisBuild,
	weights::Weight,
};
pub use hydradx_runtime::{AccountId, NativeExistentialDeposit, Treasury, VestingPalletId};
use pallet_transaction_multi_payment::Price;
use primitives::{AssetId, Balance};

use cumulus_primitives_core::ParaId;
use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
//use cumulus_primitives_core::relay_chain::AccountId;
use polkadot_primitives::v2::{BlockNumber, MAX_CODE_SIZE, MAX_POV_SIZE};
use polkadot_runtime_parachains::configuration::HostConfiguration;
use xcm_emulator::{decl_test_network, decl_test_parachain, decl_test_relay_chain};

pub const ALICE: [u8; 32] = [4u8; 32];
pub const BOB: [u8; 32] = [5u8; 32];
pub const CHARLIE: [u8; 32] = [6u8; 32];
pub const DAVE: [u8; 32] = [7u8; 32];

pub const UNITS: Balance = 1_000_000_000_000;

pub const ACALA_PARA_ID: u32 = 2_000;
pub const HYDRA_PARA_ID: u32 = 2_034;

pub const ALICE_INITIAL_NATIVE_BALANCE_ON_OTHER_PARACHAIN: Balance = 200 * UNITS;
pub const ALICE_INITIAL_NATIVE_BALANCE: Balance = 1000 * UNITS;
pub const ALICE_INITIAL_DAI_BALANCE: Balance = 200 * UNITS;
pub const BOB_INITIAL_DAI_BALANCE: Balance = 1_000 * UNITS * 1_000_000;
pub const BOB_INITIAL_NATIVE_BALANCE: Balance = 1_000 * UNITS;
pub const CHARLIE_INITIAL_LRNA_BALANCE: Balance = 1_000 * UNITS;

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;
pub const DOT: AssetId = 3;
pub const ETH: AssetId = 4;
pub const BTC: AssetId = 5;

decl_test_relay_chain! {
	pub struct PolkadotRelay {
		Runtime = polkadot_runtime::Runtime,
		XcmConfig = polkadot_runtime::xcm_config::XcmConfig,
		new_ext = polkadot_ext(),
	}
}

decl_test_parachain! {
	pub struct Hydra{
		Runtime = hydradx_runtime::Runtime,
		Origin = hydradx_runtime::Origin,
		XcmpMessageHandler = hydradx_runtime::XcmpQueue,
		DmpMessageHandler = hydradx_runtime::DmpQueue,
		new_ext = hydra_ext(),
	}
}

decl_test_parachain! {
	pub struct Acala{
		Runtime = hydradx_runtime::Runtime,
		Origin = hydradx_runtime::Origin,
		XcmpMessageHandler = hydradx_runtime::XcmpQueue,
		DmpMessageHandler = hydradx_runtime::DmpQueue,
		new_ext = acala_ext(),
	}
}

decl_test_network! {
	pub struct TestNet {
		relay_chain = PolkadotRelay,
		parachains = vec![
			(2000, Acala),
			(2034, Hydra),
		],
	}
}

fn default_parachains_host_configuration() -> HostConfiguration<BlockNumber> {
	HostConfiguration {
		minimum_validation_upgrade_delay: 5,
		validation_upgrade_cooldown: 5u32,
		validation_upgrade_delay: 5,
		code_retention_period: 1200,
		max_code_size: MAX_CODE_SIZE,
		max_pov_size: MAX_POV_SIZE,
		max_head_data_size: 32 * 1024,
		group_rotation_frequency: 20,
		chain_availability_period: 4,
		thread_availability_period: 4,
		max_upward_queue_count: 8,
		max_upward_queue_size: 1024 * 1024,
		max_downward_message_size: 1024,
		ump_service_total_weight: Weight::from_ref_time(4 * 1_000_000_000),
		max_upward_message_size: 50 * 1024,
		max_upward_message_num_per_candidate: 5,
		hrmp_sender_deposit: 0,
		hrmp_recipient_deposit: 0,
		hrmp_channel_max_capacity: 8,
		hrmp_channel_max_total_size: 8 * 1024,
		hrmp_max_parachain_inbound_channels: 4,
		hrmp_max_parathread_inbound_channels: 4,
		hrmp_channel_max_message_size: 1024 * 1024,
		hrmp_max_parachain_outbound_channels: 4,
		hrmp_max_parathread_outbound_channels: 4,
		hrmp_max_message_num_per_candidate: 5,
		dispute_period: 6,
		no_show_slots: 2,
		n_delay_tranches: 25,
		needed_approvals: 2,
		relay_vrf_modulo_samples: 2,
		zeroth_delay_tranche_width: 0,
		..Default::default()
	}
}

pub fn polkadot_ext() -> sp_io::TestExternalities {
	use polkadot_runtime::{Runtime, System};

	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![
			(AccountId::from(ALICE), 2_002 * UNITS),
			(ParaId::from(HYDRA_PARA_ID).into_account_truncating(), 10 * UNITS),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	polkadot_runtime_parachains::configuration::GenesisConfig::<Runtime> {
		config: default_parachains_host_configuration(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	<pallet_xcm::GenesisConfig as GenesisBuild<Runtime>>::assimilate_storage(
		&pallet_xcm::GenesisConfig {
			safe_xcm_version: Some(2),
		},
		&mut t,
	)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn hydra_ext() -> sp_io::TestExternalities {
	use frame_support::traits::OnInitialize;
	use hydradx_runtime::{MultiTransactionPayment, Runtime, System};

	let stable_amount = 50_000 * UNITS * 1_000_000;
	let native_amount = 936_329_588_000_000_000;
	let dot_amount = 87_719_298_250_000_u128;
	let eth_amount = 63_750_000_000_000_000_000u128;
	let btc_amount = 1_000_000_000u128;
	let omnipool_account = hydradx_runtime::Omnipool::protocol_account();

	let existential_deposit = NativeExistentialDeposit::get();

	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![
			(AccountId::from(ALICE), ALICE_INITIAL_NATIVE_BALANCE),
			(AccountId::from(BOB), BOB_INITIAL_NATIVE_BALANCE),
			(AccountId::from(CHARLIE), 1_000 * UNITS),
			(AccountId::from(DAVE), 1_000 * UNITS),
			(omnipool_account.clone(), native_amount),
			(vesting_account(), 10_000 * UNITS),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	pallet_asset_registry::GenesisConfig::<Runtime> {
		registered_assets: vec![
			(b"LRNA".to_vec(), 1_000u128, Some(LRNA)),
			(b"DAI".to_vec(), 1_000u128, Some(DAI)),
			(b"DOT".to_vec(), 1_000u128, Some(DOT)),
			(b"ETH".to_vec(), 1_000u128, Some(ETH)),
			(b"BTC".to_vec(), 1_000u128, Some(BTC)),
		],
		native_asset_name: b"HDX".to_vec(),
		native_existential_deposit: existential_deposit,
	}
	.assimilate_storage(&mut t)
	.unwrap();

	<parachain_info::GenesisConfig as GenesisBuild<Runtime>>::assimilate_storage(
		&parachain_info::GenesisConfig {
			parachain_id: HYDRA_PARA_ID.into(),
		},
		&mut t,
	)
	.unwrap();
	orml_tokens::GenesisConfig::<Runtime> {
		balances: vec![
			(AccountId::from(ALICE), LRNA, 200 * UNITS),
			(AccountId::from(ALICE), DAI, ALICE_INITIAL_DAI_BALANCE),
			(AccountId::from(BOB), LRNA, 1_000 * UNITS),
			(AccountId::from(BOB), DAI, 1_000 * UNITS * 1_000_000),
			(AccountId::from(BOB), BTC, 1_000_000),
			(AccountId::from(CHARLIE), DAI, 80_000 * UNITS * 1_000_000),
			(AccountId::from(CHARLIE), LRNA, CHARLIE_INITIAL_LRNA_BALANCE),
			(AccountId::from(DAVE), LRNA, 1_000 * UNITS),
			(AccountId::from(DAVE), DAI, 1_000 * UNITS * 1_000_000),
			(omnipool_account.clone(), DAI, stable_amount),
			(omnipool_account.clone(), ETH, eth_amount),
			(omnipool_account.clone(), BTC, btc_amount),
			(omnipool_account, DOT, dot_amount),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	<pallet_xcm::GenesisConfig as GenesisBuild<Runtime>>::assimilate_storage(
		&pallet_xcm::GenesisConfig {
			safe_xcm_version: Some(2),
		},
		&mut t,
	)
	.unwrap();

	pallet_transaction_multi_payment::GenesisConfig::<Runtime> {
		currencies: vec![
			(1, Price::from(1)),
			(DAI, Price::from(1)),
			(BTC, Price::from_inner(134000000)),
		],
		account_currencies: vec![],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	//add duster
	pallet_duster::GenesisConfig::<Runtime> {
		account_blacklist: vec![Treasury::account_id()],
		reward_account: Some(Treasury::account_id()),
		dust_account: Some(Treasury::account_id()),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	<pallet_omnipool_liquidity_mining::GenesisConfig as GenesisBuild<Runtime>>::assimilate_storage(
		&pallet_omnipool_liquidity_mining::GenesisConfig::default(),
		&mut t,
	)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
		// Make sure the prices are up-to-date.
		MultiTransactionPayment::on_initialize(1);
	});
	ext
}

#[allow(dead_code)]
pub fn apply_blocks_from_file(pallet_whitelist: Vec<&str>) {
	let blocks =
		scraper::load_blocks_snapshot::<hydradx_runtime::Block>(&std::path::PathBuf::from("../scraper/SNAPSHOT"))
			.unwrap();

	for block in blocks.iter() {
		for tx in block.extrinsics() {
			let call = &tx.function;
			let call_p = call.get_call_metadata().pallet_name;

			if pallet_whitelist.contains(&call_p) {
				let acc = &tx.signature.as_ref().unwrap().0;
				assert_ok!(call.clone().dispatch(hydradx_runtime::Origin::signed(acc.clone())));
			}
		}
	}
}

pub fn acala_ext() -> sp_io::TestExternalities {
	use hydradx_runtime::{Runtime, System};

	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![(AccountId::from(ALICE), ALICE_INITIAL_NATIVE_BALANCE_ON_OTHER_PARACHAIN)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	<parachain_info::GenesisConfig as GenesisBuild<Runtime>>::assimilate_storage(
		&parachain_info::GenesisConfig {
			parachain_id: ACALA_PARA_ID.into(),
		},
		&mut t,
	)
	.unwrap();

	<pallet_xcm::GenesisConfig as GenesisBuild<Runtime>>::assimilate_storage(
		&pallet_xcm::GenesisConfig {
			safe_xcm_version: Some(2),
		},
		&mut t,
	)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn vesting_account() -> AccountId {
	VestingPalletId::get().into_account_truncating()
}

pub fn last_hydra_events(n: usize) -> Vec<hydradx_runtime::Event> {
	frame_system::Pallet::<hydradx_runtime::Runtime>::events()
		.into_iter()
		.rev()
		.take(n)
		.rev()
		.map(|e| e.event)
		.collect()
}

pub fn expect_hydra_events(e: Vec<hydradx_runtime::Event>) {
	assert_eq!(last_hydra_events(e.len()), e);
}

pub fn set_relaychain_block_number(number: BlockNumber) {
	use frame_support::traits::OnInitialize;
	use hydradx_runtime::{Origin, ParachainSystem};

	// We need to set block number this way as well because tarpaulin code coverage tool does not like the way
	// how we set the block number with `cumulus-test-relay-sproof-builder` package
	polkadot_run_to_block(number);

	ParachainSystem::on_initialize(number);

	let (relay_storage_root, proof) = RelayStateSproofBuilder::default().into_state_root_and_proof();

	assert_ok!(ParachainSystem::set_validation_data(
		Origin::none(),
		cumulus_primitives_parachain_inherent::ParachainInherentData {
			validation_data: cumulus_primitives_core::PersistedValidationData {
				parent_head: Default::default(),
				relay_parent_number: number,
				relay_parent_storage_root: relay_storage_root,
				max_pov_size: Default::default(),
			},
			relay_chain_state: proof,
			downward_messages: Default::default(),
			horizontal_messages: Default::default(),
		}
	));
}
pub fn polkadot_run_to_block(to: BlockNumber) {
	use frame_support::traits::{OnFinalize, OnInitialize};
	while hydradx_runtime::System::block_number() < to {
		let b = hydradx_runtime::System::block_number();

		hydradx_runtime::System::on_finalize(b);
		hydradx_runtime::MultiTransactionPayment::on_finalize(b);
		hydradx_runtime::EmaOracle::on_finalize(b);
		hydradx_runtime::DCA::on_finalize(b);
		hydradx_runtime::CircuitBreaker::on_finalize(b);

		hydradx_runtime::System::on_initialize(b + 1);
		hydradx_runtime::MultiTransactionPayment::on_initialize(b + 1);
		hydradx_runtime::EmaOracle::on_initialize(b + 1);
		hydradx_runtime::DCA::on_initialize(b + 1);
		hydradx_runtime::CircuitBreaker::on_initialize(b + 1);

		hydradx_runtime::System::set_block_number(b + 1);
	}
}

pub fn hydra_live_ext(path_to_snapshot: &str) -> sp_io::TestExternalities {
	let ext = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.unwrap()
		.block_on(async {
			use remote_externalities::*;

			let snapshot_config = SnapshotConfig::from(String::from(path_to_snapshot));
			let offline_config = OfflineConfig {
				state_snapshot: snapshot_config,
			};
			let mode = Mode::Offline(offline_config);

			let builder = Builder::<hydradx_runtime::Block>::new().mode(mode);

			builder.build().await.unwrap()
		});
	ext
}

pub fn init_omnipool() {
	let native_price = FixedU128::from_inner(1201500000000000);
	let stable_price = FixedU128::from_inner(45_000_000_000);

	assert_ok!(hydradx_runtime::Omnipool::set_tvl_cap(
		hydradx_runtime::Origin::root(),
		522_222_000_000_000_000_000_000,
	));

	assert_ok!(hydradx_runtime::Omnipool::initialize_pool(
		hydradx_runtime::Origin::root(),
		stable_price,
		native_price,
		Permill::from_percent(100),
		Permill::from_percent(10)
	));
}
