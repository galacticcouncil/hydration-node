//! Is ICE's flat `EXTRA_GAS` enough for a solution whose intents move Aave aTokens?
//!
//! `pallet_dispatcher::ExtraGas` is one pool drained by every EVM call of the solution
//! (`evm/executor.rs` adds it to each call's limit, then subtracts what was used), not a
//! per-call allowance. Settlement moves each intent's input twice — unlock, then pot-in —
//! so an aToken batch spends it per intent.

use crate::driver::HydrationTestDriver;
use crate::polkadot_test_net::*;
use frame_support::traits::ExistenceRequirement;
use frame_support::{assert_ok, BoundedVec};
use hydradx_runtime::{Currencies, Dispatcher, Intent, Router, RuntimeOrigin};
use hydradx_traits::evm::ExtraGasSupport;
use hydradx_traits::router::{PoolType, Trade};
use ice_support::{IntentDataInput, SwapParams};
use orml_traits::MultiCurrency;
use primitives::{AccountId, AssetId, Balance};

const PATH_TO_SNAPSHOT: &str = "snapshots/ice/mainnet_heavy";

const HDX: AssetId = 0;

/// What `pallet_ice` grants a whole solution (`pallets/ice/src/lib.rs`).
const ICE_EXTRA_GAS: u64 = 1_000_000;
/// What a bare erc20 transfer gets (`runtime/hydradx/src/evm/erc20_currency.rs`).
const ERC20_GAS_LIMIT: u64 = 400_000;
/// What `get_reserves_list` gets (`runtime/hydradx/src/evm/aave_trade_executor.rs`).
const RESERVES_LIST_GAS_LIMIT: u64 = 1_000_000;
/// Headroom while measuring, so nothing reverts before we read the meter.
const PROBE_GAS: u64 = 8_000_000;

/// (mintable underlying, its aToken) — every underlying here is an orml `Token`.
const AAVE_PAIRS: [(AssetId, AssetId); 6] = [
	(5, 1001),         // DOT   -> aDOT
	(1_000_765, 1006), // tBTC  -> atBTC
	(39, 1039),        // PAXG  -> aPAXG
	(34, 1007),        // ETH   -> aETH
	(19, 1004),        // WBTC  -> aWBTC
	(15, 1005),        // vDOT  -> avDOT
];

/// An EVM address is the account id's first 20 bytes, so test accounts have to differ *there*:
/// varying a trailing byte gives every one of them the same `H160`, and aToken self-transfers
/// take a different path inside Aave than real ones.
fn account(tag: u8, i: u8) -> AccountId {
	let mut raw = [0u8; 32];
	raw[0] = 0xce;
	raw[1] = tag;
	raw[2] = i;
	AccountId::from(raw)
}

/// Gas consumed above the base limits of whatever EVM calls `f` makes.
fn measure<R>(f: impl FnOnce() -> R) -> (R, u64) {
	Dispatcher::set_extra_gas(PROBE_GAS);
	let out = f();
	let left = Dispatcher::extra_gas();
	Dispatcher::clear_extra_gas();
	(out, PROBE_GAS.saturating_sub(left))
}

/// A supply amount that clears the asset's ED regardless of its decimals.
fn mint_unit(asset: AssetId) -> Balance {
	10u128.pow(
		<Currencies as MultiCurrency<AccountId>>::minimum_balance(asset)
			.checked_ilog10()
			.unwrap_or(6)
			+ 2,
	)
}

fn mint_atoken(who: &AccountId, underlying: AssetId, atoken: AssetId, amount: Balance) -> sp_runtime::DispatchResult {
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		who.clone(),
		HDX,
		(100 * UNITS) as i128
	));
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		who.clone(),
		underlying,
		amount as i128
	));
	let (result, _) = measure(|| {
		Router::sell(
			RuntimeOrigin::signed(who.clone()),
			underlying,
			atoken,
			amount,
			0,
			BoundedVec::truncate_from(vec![Trade {
				pool: PoolType::Aave,
				asset_in: underlying,
				asset_out: atoken,
			}]),
		)
	});
	// A frozen reserve (Aave error 28) can no longer be supplied into — nothing to measure.
	result
}

fn submit_swap_intent(who: &AccountId, asset_in: AssetId, amount_in: Balance) -> sp_runtime::DispatchResult {
	// amount_out must clear the out-asset ED, or the intent is rejected before any EVM call.
	let amount_out = <Currencies as MultiCurrency<AccountId>>::minimum_balance(HDX);
	Intent::submit_intent(
		RuntimeOrigin::signed(who.clone()),
		pallet_intent::types::IntentInput {
			data: IntentDataInput::Swap(SwapParams {
				asset_in,
				asset_out: HDX,
				amount_in,
				amount_out,
				partial: false,
			}),
			deadline: None,
			on_resolved: None,
		},
	)
}

/// How much gas an intent on each aToken needs beyond the bare erc20 limit.
#[test]
#[ignore = "needs the mainnet_heavy snapshot"]
fn intent_creation_gas_should_be_measured_for_every_atoken() {
	HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		println!(
			"{:>8} {:>10} {:>12} {:>14}",
			"aToken", "transfer", "submit_intent", "intents in 1M"
		);
		for (i, (underlying, atoken)) in AAVE_PAIRS.into_iter().enumerate() {
			let holder = account(1, i as u8);
			let peer = account(2, i as u8);
			let unit = 10u128.pow(
				<Currencies as MultiCurrency<AccountId>>::minimum_balance(underlying)
					.checked_ilog10()
					.unwrap_or(6) + 2,
			);
			if let Err(e) = mint_atoken(&holder, underlying, atoken, 1_000 * unit) {
				println!("{atoken:>8}   skipped: cannot supply {underlying} ({e:?})");
				continue;
			}
			let held = <Currencies as MultiCurrency<AccountId>>::free_balance(atoken, &holder);
			assert!(held > 0, "asset {atoken}: no aTokens minted");

			// Fresh recipient: the cold-storage case a first-time transfer pays for.
			let (transfer, transfer_gas) = measure(|| {
				<Currencies as MultiCurrency<AccountId>>::transfer(
					atoken,
					&holder,
					&peer,
					held / 4,
					ExistenceRequirement::AllowDeath,
				)
			});
			assert_ok!(transfer);

			let (submitted, submit_gas) = measure(|| submit_swap_intent(&holder, atoken, held / 4));
			assert_ok!(submitted);

			let per_intent = submit_gas.max(1);
			println!(
				"{atoken:>8} {:>10} {:>13} {:>14}",
				transfer_gas,
				submit_gas,
				ICE_EXTRA_GAS / per_intent
			);
		}
	});
}

/// Creation with no allowance at all — pallet-intent sets none of its own.
#[test]
#[ignore = "needs the mainnet_heavy snapshot"]
fn submit_intent_should_report_its_result_without_extra_gas() {
	HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		for (i, (underlying, atoken)) in AAVE_PAIRS.into_iter().enumerate() {
			let holder = account(3, i as u8);
			if mint_atoken(&holder, underlying, atoken, 1_000 * mint_unit(underlying)).is_err() {
				continue;
			}
			let held = <Currencies as MultiCurrency<AccountId>>::free_balance(atoken, &holder);
			println!(
				"{atoken}: submit_intent = {:?}",
				submit_swap_intent(&holder, atoken, held / 2)
			);
		}
	});
}

/// Walks the batch size up until the shared allowance runs out.
#[test]
#[ignore = "needs the mainnet_heavy snapshot"]
fn submit_solution_should_survive_a_batch_of_atoken_intents() {
	const UNDERLYING: AssetId = 5;
	const ATOKEN: AssetId = 1001;

	for count in [1u8, 4, 8, 16] {
		let driver = HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
		driver.execute(|| {
			for i in 0..count {
				let who = account(4, i);
				assert_ok!(mint_atoken(&who, UNDERLYING, ATOKEN, 1_000 * mint_unit(UNDERLYING)));
				let held = <Currencies as MultiCurrency<AccountId>>::free_balance(ATOKEN, &who);
				Dispatcher::set_extra_gas(ICE_EXTRA_GAS);
				assert_ok!(submit_swap_intent(&who, ATOKEN, held / 2));
				Dispatcher::clear_extra_gas();
			}
		});
		println!("=== {count} aToken intents: {:?}", driver.try_run_solver());
	}
	let _ = ERC20_GAS_LIMIT;
}

/// The solver's whole view of Aave. `pairs()` swallows a gas failure and returns an empty list
/// (`ice_simulator_provider.rs:134`), so a too-low view limit silently removes every Aave route.
#[test]
#[ignore = "needs the mainnet_heavy snapshot"]
fn aave_pairs_should_be_visible_to_the_solver() {
	use amm_simulator::aave::Simulator as AaveSimulator;
	use hydradx_runtime::ice_simulator_provider;
	use hydradx_traits::amm::AmmSimulator;

	HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let via_api =
			hydradx_runtime::evm::aave_trade_executor::AaveTradeExecutor::<hydradx_runtime::Runtime>::get_reserves_list(
				pallet_liquidation::BorrowingContract::<hydradx_runtime::Runtime>::get(),
			);
		println!("get_reserves_list: {:?}", via_api.as_ref().map(|r| r.len()));

		let snapshot = AaveSimulator::<ice_simulator_provider::Aave<hydradx_runtime::Runtime>>::snapshot();
		// `reserves` is filled by the simulator's own 1M-gas read; `pairs` comes through
		// AaveTradeExecutor at its far lower limit. They can disagree.
		println!(
			"solver sees {} aave pairs, snapshot carries {} reserves",
			snapshot.pairs.len(),
			snapshot.reserves.len()
		);
		let mut listed = snapshot.pairs.clone();
		listed.sort();
		println!("RUNTIME_PAIRS {listed:?}");
		assert!(!snapshot.pairs.is_empty(), "solver sees no aave pairs at all");
	});
}

/// `getReservesList()` walks every reserve, so its cost grows with the market. Ladders the view
/// limit to find what it actually costs here, and how much room the configured limit leaves.
#[test]
#[ignore = "needs the mainnet_heavy snapshot"]
fn get_reserves_list_should_have_headroom_in_the_view_gas_limit() {
	use hydradx_runtime::evm::precompiles::handle::EvmDataWriter;
	use hydradx_traits::evm::{CallContext, EVM};
	use liquidation_worker_support::Function;
	use pallet_evm::ExitReason::Succeed;

	HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let pool = pallet_liquidation::BorrowingContract::<hydradx_runtime::Runtime>::get();
		// Read the count with a deliberately generous limit: the configured one may be too low,
		// which is exactly the failure being measured.
		let probe = {
			let data = EvmDataWriter::new_with_selector(Function::GetReservesList).build();
			hydradx_runtime::evm::Executor::<hydradx_runtime::Runtime>::view(CallContext::new_view(pool), data, 5_000_000)
		};
		assert!(matches!(probe.exit_reason, Succeed(_)), "reserves list unreadable even at 5M");
		let reserve_count_words = &probe.value[32..64];
		let reserves = vec![(); sp_core::U256::from_big_endian(reserve_count_words).as_usize()];
		println!(
			"configured get_reserves_list: {:?}",
			hydradx_runtime::evm::aave_trade_executor::AaveTradeExecutor::<hydradx_runtime::Runtime>::get_reserves_list(pool).map(|r| r.len())
		);

		let call = |limit: u64| {
			let data = EvmDataWriter::new_with_selector(Function::GetReservesList).build();
			let result = hydradx_runtime::evm::Executor::<hydradx_runtime::Runtime>::view(
				CallContext::new_view(pool),
				data,
				limit,
			);
			matches!(result.exit_reason, Succeed(_))
		};

		let (mut low, mut high) = (21_000u64, 400_000u64);
		assert!(call(high), "getReservesList fails even at {high}");
		while high - low > 500 {
			let mid = (low + high) / 2;
			if call(mid) {
				high = mid;
			} else {
				low = mid;
			}
		}
		let reserve_count = reserves.len() as u64;
		let per_reserve = high / reserve_count;
		println!("getReservesList costs ~{high} gas at {reserve_count} reserves (~{per_reserve} per reserve)");
		println!("that ceiling is reached at {} reserves", RESERVES_LIST_GAS_LIMIT / per_reserve);

		// getReserveData is a single-reserve read on the same VIEW_GAS_LIMIT the trade path uses.
		let data_cost = |asset: sp_core::H160| {
			let call = |limit: u64| {
				let mut d = EvmDataWriter::new_with_selector(Function::GetReserveData).build();
				d.extend_from_slice(sp_core::H256::from(asset).as_bytes());
				let r = hydradx_runtime::evm::Executor::<hydradx_runtime::Runtime>::view(
					CallContext::new_view(pool),
					d,
					limit,
				);
				matches!(r.exit_reason, Succeed(_))
			};
			let (mut low, mut high) = (21_000u64, 400_000u64);
			if !call(high) {
				return None;
			}
			while high - low > 500 {
				let mid = (low + high) / 2;
				if call(mid) { high = mid } else { low = mid }
			}
			Some(high)
		};
		let addrs = {
			let d = EvmDataWriter::new_with_selector(Function::GetReservesList).build();
			let r = hydradx_runtime::evm::Executor::<hydradx_runtime::Runtime>::view(CallContext::new_view(pool), d, 5_000_000);
			r.value[64..].chunks(32).map(|w| sp_core::H160::from_slice(&w[12..32])).collect::<Vec<_>>()
		};
		let worst = addrs.iter().filter_map(|a| data_cost(*a)).max().unwrap_or(0);
		println!("getReserveData worst of {} reserves: {worst} gas (VIEW_GAS_LIMIT is what the trade path uses)", addrs.len());


		// The cost grows with every listing; a silent `Err(_) => vec![]` in
		// `ice_simulator_provider::pairs()` is what a breach looks like, so warn while there is
		// still room to react.
		assert!(
			high * 2 < RESERVES_LIST_GAS_LIMIT,
			"getReservesList costs {high} of its {RESERVES_LIST_GAS_LIMIT} limit at {reserve_count} 			 reserves - less than 2x headroom before the solver silently loses every aave route"
		);
	});
}

/// Aave's `finalizeTransfer` revalidates the health factor when the sender is using the aToken as
/// collateral *and* owes anything. `calculateUserAccountData` walks the *user's* config bitmap, so
/// the cost tracks how many reserves that account touches - not how many the market has.
///
/// Measured here (2026-09-17, mainnet_sep): 143_911 gas with no debt at any collateral count,
/// then 218_823 with debt at one collateral reserve and ~40_000 for each further one. The
/// `Erc20Currency` limit is 400_000, so an indebted account tips over at its 6th collateral
/// reserve and can no longer move its aTokens through any substrate path - `submit_intent` and
/// DCA migration included. Schedule 35315's owner holds 7 collateral reserves and 1 borrow.
#[test]
#[ignore = "needs the mainnet_heavy snapshot"]
fn atoken_transfer_cost_should_track_the_owners_aave_footprint() {
	use crate::liquidation::{borrow, supply};
	use hydradx_runtime::evm::precompiles::erc20_mapping::HydraErc20Mapping;
	use hydradx_runtime::evm::Executor;
	use hydradx_runtime::{AssetRegistry, EVMAccounts, Runtime};
	use hydradx_traits::evm::{CallContext, Erc20Encoding, EVM};
	use hydradx_traits::BoundErc20;
	use pallet_evm::ExitReason::Succeed;
	use sp_core::U256 as EvmU256;
	use sp_core::{H256, U256};

	const DOT: AssetId = 5;
	const ADOT: AssetId = 1001;

	/// Smallest gas limit an erc20 `transfer` of `amount` succeeds at.
	fn transfer_cost(asset: AssetId, from: &AccountId, to: &AccountId, amount: Balance) -> u64 {
		// `Erc20Currency` calls the *bound* contract; the erc20-mapping address is a precompile
		// and answers a transfer far more cheaply than the real aToken does.
		let contract = <AssetRegistry as BoundErc20>::contract_address(asset)
			.unwrap_or_else(|| HydraErc20Mapping::encode_evm_address(asset));
		let sender = EVMAccounts::evm_address(from);
		let recipient = EVMAccounts::evm_address(to);
		let attempt = |limit: u64| {
			let mut data = sp_io::hashing::keccak_256(b"transfer(address,uint256)")[..4].to_vec();
			data.extend_from_slice(H256::from(recipient).as_bytes());
			data.extend_from_slice(&EvmU256::from(amount).to_big_endian());
			let result = frame_support::storage::with_transaction(|| {
				let call =
					Executor::<Runtime>::call(CallContext::new_call(contract, sender), data, U256::zero(), limit);
				sp_runtime::TransactionOutcome::Rollback(Ok::<_, sp_runtime::DispatchError>(call))
			})
			.expect("rollback keeps the measurement side-effect free");
			matches!(result.exit_reason, Succeed(_))
		};

		let (mut low, mut high) = (21_000u64, 3_000_000u64);
		assert!(attempt(high), "transfer of {asset} fails even at {high}");
		while high - low > 1_000 {
			let mid = (low + high) / 2;
			if attempt(mid) {
				high = mid;
			} else {
				low = mid;
			}
		}
		high
	}

	HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let who = account(9, 0);
		let peer = account(9, 1);
		let pool = pallet_liquidation::BorrowingContract::<Runtime>::get();
		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool));

		for holder in [&who, &peer] {
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				holder.clone(),
				HDX,
				(1_000 * UNITS) as i128
			));
			assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(holder.clone())));
		}
		let evm = EVMAccounts::evm_address(&who);

		// Collateral in one reserve at a time; the transfer cost is re-measured after each.
		let mut supplied = 0;
		let mut debt_base = 0u64;
		for (underlying, atoken) in AAVE_PAIRS {
			let unit = mint_unit(underlying);
			if Currencies::update_balance(RuntimeOrigin::root(), who.clone(), underlying, (20_000 * unit) as i128)
				.is_err()
			{
				continue;
			}
			let contract = HydraErc20Mapping::encode_evm_address(underlying);
			let before = <Currencies as MultiCurrency<AccountId>>::free_balance(atoken, &who);
			let supplied_ok = frame_support::storage::with_transaction(|| {
				let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
					supply(pool, evm, contract, 10_000 * unit)
				}));
				match outcome {
					Ok(()) => sp_runtime::TransactionOutcome::Commit(Ok::<_, sp_runtime::DispatchError>(true)),
					Err(_) => sp_runtime::TransactionOutcome::Rollback(Ok(false)),
				}
			})
			.unwrap_or(false);
			if !supplied_ok || <Currencies as MultiCurrency<AccountId>>::free_balance(atoken, &who) == before {
				println!("       skipped reserve {underlying}");
				continue;
			}
			supplied += 1;

			// Borrow once, against the first collateral: everything after this is measured with
			// the health-factor revalidation in play, which is the path an indebted owner takes.
			if supplied == 1 {
				borrow(
					pool,
					evm,
					HydraErc20Mapping::encode_evm_address(DOT),
					500 * mint_unit(DOT),
				);
			}

			let held = <Currencies as MultiCurrency<AccountId>>::free_balance(ADOT, &who);
			if held > 0 {
				let cost = transfer_cost(ADOT, &who, &peer, held / 64);
				if supplied == 1 {
					debt_base = cost;
				}
				println!("{supplied} collateral reserve(s) + debt: aDOT transfer costs {cost} of {ERC20_GAS_LIMIT}");
			}
		}

		let held = <Currencies as MultiCurrency<AccountId>>::free_balance(ADOT, &who);
		let with_debt = transfer_cost(ADOT, &who, &peer, held / 64);
		let per_reserve = with_debt.saturating_sub(debt_base) / (supplied.max(2) - 1);
		println!("final: {supplied} collateral reserve(s) + 1 debt costs {with_debt} of {ERC20_GAS_LIMIT}");
		let fits = (ERC20_GAS_LIMIT - debt_base) / per_reserve.max(1) + 1;
		println!(
			"~{per_reserve} per extra reserve: {fits} collateral reserves still fit, {} does not",
			fits + 1
		);

		// The unreserve leg sends *from* the pallet's reserve account, which carries no debt of its
		// own, so a refund or cancellation should never take the expensive path. Approximated here
		// with a debt-free sender paying the indebted account.
		let peer_unit = mint_unit(DOT);
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			peer.clone(),
			DOT,
			(20_000 * peer_unit) as i128
		));
		supply(
			pool,
			EVMAccounts::evm_address(&peer),
			HydraErc20Mapping::encode_evm_address(DOT),
			10_000 * peer_unit,
		);
		let peer_held = <Currencies as MultiCurrency<AccountId>>::free_balance(ADOT, &peer);
		let inbound = transfer_cost(ADOT, &peer, &who, peer_held / 64);
		println!("inbound to the indebted account (unreserve direction): {inbound} of {ERC20_GAS_LIMIT}");

		// A second borrowed reserve: `calculateUserAccountData` walks debt positions too.
		let mut second_debt = None;
		for (underlying, _) in AAVE_PAIRS.into_iter().filter(|(u, _)| *u != DOT) {
			let borrowed = frame_support::storage::with_transaction(|| {
				let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
					borrow(
						pool,
						evm,
						HydraErc20Mapping::encode_evm_address(underlying),
						mint_unit(underlying) / 10,
					)
				}));
				match outcome {
					Ok(()) => sp_runtime::TransactionOutcome::Commit(Ok::<_, sp_runtime::DispatchError>(true)),
					Err(_) => sp_runtime::TransactionOutcome::Rollback(Ok(false)),
				}
			})
			.unwrap_or(false);
			if borrowed {
				second_debt = Some(underlying);
				break;
			}
		}
		match second_debt {
			Some(asset) => {
				let held = <Currencies as MultiCurrency<AccountId>>::free_balance(ADOT, &who);
				let two = transfer_cost(ADOT, &who, &peer, held / 64);
				println!("{supplied} collateral + 2 borrowed reserves (added {asset}): {two} of {ERC20_GAS_LIMIT}");
			}
			None => println!("no second reserve could be borrowed on this snapshot"),
		}

		// Every aToken this account holds, at its full footprint: is any already over the limit?
		let mut over_limit = None;
		for (_, atoken) in AAVE_PAIRS {
			let bal = <Currencies as MultiCurrency<AccountId>>::free_balance(atoken, &who);
			if bal == 0 {
				continue;
			}
			let cost = transfer_cost(atoken, &who, &peer, bal / 64);
			println!("  {atoken}: {cost} of {ERC20_GAS_LIMIT}{}", if cost > ERC20_GAS_LIMIT { "  OVER" } else { "" });
			if cost > ERC20_GAS_LIMIT && over_limit.is_none() {
				over_limit = Some((atoken, bal));
			}
		}

		// `dispatch_with_extra_gas` sets the pool and kills it afterwards; pallet-intent never
		// touches it, so nothing inside `submit_intent` can clobber the caller's allowance.
		if let Some((atoken, bal)) = over_limit {
			let bare = <Currencies as MultiCurrency<AccountId>>::transfer(
				atoken,
				&who,
				&peer,
				bal / 64,
				ExistenceRequirement::AllowDeath,
			);
			println!("bare transfer of {atoken}: {bare:?}");

			let wrapped = Dispatcher::dispatch_with_extra_gas(
				RuntimeOrigin::signed(who.clone()),
				Box::new(hydradx_runtime::RuntimeCall::Currencies(
					pallet_currencies::Call::transfer {
						dest: peer.clone(),
						currency_id: atoken,
						amount: bal / 64,
					},
				)),
				1_000_000,
			);
			println!("same transfer under dispatch_with_extra_gas: {:?}", wrapped.map(|_| ()));
			assert_eq!(Dispatcher::extra_gas(), 0, "extra gas must not leak past the dispatch");
		} else {
			println!("no aToken exceeds the limit at this footprint - needs a heavier account");
		}


		assert!(
			with_debt < ERC20_GAS_LIMIT,
			"an aToken transfer no longer fits the erc20 gas limit: {with_debt} > {ERC20_GAS_LIMIT}"
		);
	});
}

/// `dispatch_with_extra_gas` writes the pool with `set` and wipes it with `kill`, so it is not
/// additive and does not restore what was there before. Nothing inside `submit_intent` touches the
/// pool today, but anything that does would clobber an outer allowance the same way.
#[test]
#[ignore = "needs the mainnet_heavy snapshot"]
fn extra_gas_should_not_survive_a_nested_dispatch() {
	use hydradx_runtime::{Dispatcher, Runtime, RuntimeCall};

	HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let who = account(7, 0);
		let peer = account(7, 1);
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			who.clone(),
			HDX,
			(1_000 * UNITS) as i128
		));

		// An ambient allowance, as an outer caller would leave it.
		Dispatcher::set_extra_gas(8_000_000);
		assert_eq!(Dispatcher::extra_gas(), 8_000_000);

		assert_ok!(Dispatcher::dispatch_with_extra_gas(
			RuntimeOrigin::signed(who.clone()),
			Box::new(RuntimeCall::Currencies(pallet_currencies::Call::transfer {
				dest: peer,
				currency_id: HDX,
				amount: UNITS,
			})),
			1_000_000,
		));

		// Not 8_000_000, and not 7_000_000 either: the nested dispatch killed it outright.
		assert_eq!(
			Dispatcher::extra_gas(),
			0,
			"a nested dispatch_with_extra_gas destroys the caller's allowance instead of restoring it"
		);
		let _: fn() = || { let _ = pallet_intent::Pallet::<Runtime>::intent_owner(0u128); };
	});
}

/// The node-side repair: with the runtime still shipping an empty venue list, derive the pairs from
/// the reserve data the snapshot already carries and check the result matches what a working
/// `pairs()` returns. Mirrors `repair_aave_venues` in `node/src/ice_solver_worker.rs`.
#[test]
#[ignore = "needs the mainnet_heavy snapshot"]
fn node_side_repair_should_reproduce_the_runtime_venue_list() {
	use amm_simulator::aave::Simulator as AaveSimulator;
	use hydradx_runtime::evm::precompiles::erc20_mapping::HydraErc20Mapping;
	use hydradx_runtime::{ice_simulator_provider, Runtime};
	use hydradx_traits::amm::AmmSimulator;
	use hydradx_traits::evm::Erc20Mapping;

	HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let mut snapshot = AaveSimulator::<ice_simulator_provider::Aave<Runtime>>::snapshot();
		println!(
			"as shipped: {} pairs, {} reserves",
			snapshot.pairs.len(),
			snapshot.reserves.len()
		);

		// Exactly what the node does after decoding the shipped snapshot.
		if snapshot.pairs.is_empty() && !snapshot.reserves.is_empty() {
			snapshot.pairs = snapshot
				.reserves
				.iter()
				.filter_map(|(underlying, reserve)| {
					Some((*underlying, HydraErc20Mapping::address_to_asset(reserve.atoken_address)?))
				})
				.collect();
		}
		println!("after repair: {} pairs", snapshot.pairs.len());

		// A rebuilt venue list is only useful if it names the same venues the chain would.
		let mut rebuilt = snapshot.pairs.clone();
		rebuilt.sort();
		let mut expected: Vec<_> = snapshot
			.reserves
			.keys()
			.filter_map(|underlying| {
				let reserve = snapshot.reserves.get(underlying)?;
				Some((*underlying, HydraErc20Mapping::address_to_asset(reserve.atoken_address)?))
			})
			.collect();
		expected.sort();
		assert_eq!(rebuilt, expected, "rebuilt venue list diverges from the reserve map");

		// The list a working `pairs()` returns on this snapshot, captured with the runtime gas fix
		// in place. The node's reconstruction has to match it exactly - a venue it invents is a
		// solution the chain rejects, and a venue it drops is a route silently lost.
		//
		// 26 from 27 reserves: HOLLAR's aToken (0x8c0f3b96..5b108e) is not a registered asset, so
		// the runtime drops that reserve too. Not a defect of the repair.
		const RUNTIME_PAIRS: [(AssetId, AssetId); 26] = [
			(5, 1001), (10, 1002), (15, 1005), (19, 1004), (22, 1003), (34, 1007), (39, 1039),
			(43, 1043), (44, 1044), (46, 1046), (103, 1008), (105, 1105), (110, 1110), (111, 1111),
			(112, 1112), (113, 1113), (143, 1143), (146, 1146), (690, 69), (816, 1816), (4200, 420),
			(10044, 4444), (10055, 11055), (90001, 9001), (1000752, 1009), (1000765, 1006),
		];
		assert_eq!(
			rebuilt,
			RUNTIME_PAIRS.to_vec(),
			"node-side venue list differs from what a working runtime pairs() returns"
		);

		for (underlying, reserve) in &snapshot.reserves {
			if HydraErc20Mapping::address_to_asset(reserve.atoken_address).is_none() {
				println!("  reserve {underlying}: atoken {:?} has no asset id", reserve.atoken_address);
			}
		}
	});
}

/// Can a binary built from this tree decode what mainnet's runtime actually ships?
///
/// `ReserveData` is `Encode`/`Decode` and gained `available_liquidity` after spec 443 was first
/// cut. If the deployed runtime predates that field, a node built here fails the snapshot decode
/// at `ice_solver_worker.rs:133` and produces no solution at all - strictly worse than the empty
/// aave venue list it was meant to fix. Blob captured live from mainnet via
/// `state_call("IceSolverApi_solver_input", "0x")`.
#[test]
#[ignore = "needs a live mainnet solver_input blob"]
fn this_tree_should_decode_what_mainnet_ships() {
	use codec::Decode;
	use hydradx_runtime::HydrationSimulators;
	use hydradx_traits::amm::SimulatorSet;
	use pallet_ice_runtime_api::SolverInput;

	let path = std::env::var("SOLVER_INPUT_HEX").unwrap_or_else(|_| {
		"/tmp/claude-501/-Volumes-T9-workspace-gc-hydration-node/9cf7e935-8875-44d7-a8d2-0e1a57540959/scratchpad/mainnet_solver_input.hex".into()
	});
	let hex = std::fs::read_to_string(&path).expect("blob file");
	let bytes = hex::decode(hex.trim().trim_start_matches("0x")).expect("hex");

	let input = Option::<SolverInput>::decode(&mut &bytes[..]).expect("SolverInput envelope decodes");
	let input = input.expect("mainnet returned a solver input");
	println!("intents: {}, state bytes: {}", input.intents.len(), input.state.len());

	match <HydrationSimulators as SimulatorSet>::State::decode(&mut &input.state[..]) {
		Ok(state) => {
			println!("DECODED OK - aave reserves: {}, pairs: {}", state.2.reserves.len(), state.2.pairs.len());
		}
		Err(e) => panic!("this tree cannot decode mainnet's snapshot: {e:?}"),
	}
}

/// The real owner of the two stuck aToken DCAs holds 15 collateral reserves and 8 borrows, so each
/// of their aToken transfers pays Aave's health-factor walk. Settlement moves each intent's input
/// twice, all sharing ICE's single 1M allowance - so the question is not whether one transfer fits
/// but whether the batch does.
#[test]
#[ignore = "needs the mainnet_heavy snapshot"]
fn stuck_dca_owner_transfer_cost_should_fit_the_ice_allowance() {
	use hydradx_runtime::evm::precompiles::erc20_mapping::HydraErc20Mapping;
	use hydradx_runtime::evm::Executor;
	use hydradx_runtime::{AssetRegistry, EVMAccounts, Runtime};
	use hydradx_traits::evm::{CallContext, EVM};
	use hydradx_traits::evm::Erc20Encoding;
	use hydradx_traits::BoundErc20;
	use pallet_evm::ExitReason::Succeed;
	use sp_core::{H256, U256};

	// 135yiujiLFfogvTwbfr3yoqGK7zAu3f5SD5y1q8PMokYeSuc
	const OWNER_EVM: [u8; 20] = hex_literal::hex!("5c4464cb3a63c97b065d8576533a3b6c14f9fc52");
	const ASSETS: [AssetId; 3] = [1112, 1006, 1003]; // HUSDS, atBTC, aUSDC

	HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let sender = sp_core::H160(OWNER_EVM);
		let recipient = EVMAccounts::evm_address(&account(5, 1));
		let mut total = 0u64;

		for asset in ASSETS {
			let held = <Currencies as MultiCurrency<AccountId>>::free_balance(asset, &{
				use hydradx_traits::evm::InspectEvmAccounts;
				EVMAccounts::account_id(sender)
			});
			if held == 0 {
				println!("{asset}: owner holds none, skipping");
				continue;
			}
			let contract = <AssetRegistry as BoundErc20>::contract_address(asset)
				.unwrap_or_else(|| HydraErc20Mapping::encode_evm_address(asset));
			let attempt = |limit: u64| {
				let mut data = sp_io::hashing::keccak_256(b"transfer(address,uint256)")[..4].to_vec();
				data.extend_from_slice(H256::from(recipient).as_bytes());
				data.extend_from_slice(&U256::from(held / 64).to_big_endian());
				let r = frame_support::storage::with_transaction(|| {
					let call = Executor::<Runtime>::call(CallContext::new_call(contract, sender), data, U256::zero(), limit);
					sp_runtime::TransactionOutcome::Rollback(Ok::<_, sp_runtime::DispatchError>(call))
				})
				.expect("rollback");
				matches!(r.exit_reason, Succeed(_))
			};
			let (mut low, mut high) = (21_000u64, 4_000_000u64);
			if !attempt(high) {
				println!("{asset}: transfer fails even at {high}");
				continue;
			}
			while high - low > 2_000 {
				let mid = (low + high) / 2;
				if attempt(mid) { high = mid } else { low = mid }
			}
			let overage = high.saturating_sub(ERC20_GAS_LIMIT);
			total += overage;
			println!("{asset}: transfer costs {high} (base {ERC20_GAS_LIMIT}, draws {overage} from the pool)");
		}
		println!("two pot-in transfers would draw ~{} of ICE's {ICE_EXTRA_GAS} allowance", total);
	});
}
