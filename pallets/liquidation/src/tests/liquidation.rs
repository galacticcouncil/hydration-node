// we don't need to run tests with benchmarking feature
#![cfg(not(feature = "runtime-benchmarks"))]
#![allow(clippy::bool_assert_comparison)]

pub use crate::tests::mock::*;
use crate::weights::WeightInfo;
use crate::Call;
use crate::{
	pallet, Error, Event, Route, BASE_UNSIGNED_LIQUIDATION_PRIORITY, MAX_UNSIGNED_LIQUIDATION_LIFETIME,
	MAX_UNSIGNED_LIQUIDATION_PRIORITY,
};
use codec::Encode;
use frame_support::dispatch::{DispatchResultWithPostInfo, GetDispatchInfo};
use frame_support::pallet_prelude::{InvalidTransaction, TransactionSource};
use frame_support::weights::Weight;
use frame_support::{assert_noop, assert_ok};
use hydradx_traits::{
	evm::InspectEvmAccounts,
	router::{AssetPair, RouteProvider},
};
use orml_traits::parameters::sp_runtime::BoundedVec;
use orml_traits::MultiCurrency;

pub fn expect_last_events(e: Vec<RuntimeEvent>) {
	// We only check if the events are as expected, not necessarily in order.
	for event in e {
		test_utils::expect_event::<RuntimeEvent, Test>(event);
	}
}
use frame_support::pallet_prelude::ValidTransaction;
use frame_support::pallet_prelude::ValidateUnsigned;
use primitives::EvmAddress;

#[test]
fn liquidation_should_transfer_profit_to_treasury() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			EvmAddress::from_slice(&[9; 20])
		));
		let bob_evm_address = EvmAccounts::evm_address(&BOB);
		let debt_to_cover = 1_000 * ONE;

		let route = Router::get_route(AssetPair {
			asset_in: HDX,
			asset_out: DOT,
		});

		let hdx_total_issuance = Currencies::total_issuance(HDX);
		let dot_total_issuance = Currencies::total_issuance(DOT);

		let hdx_alice_balance_before = Currencies::free_balance(HDX, &ALICE);
		let dot_alice_balance_before = Currencies::free_balance(DOT, &ALICE);

		assert!(Currencies::free_balance(HDX, &Liquidation::account_id()) == 0);
		assert!(Currencies::free_balance(DOT, &Liquidation::account_id()) == 0);

		let hdx_contract_balance_before = Currencies::free_balance(HDX, &MONEY_MARKET);
		let dot_contract_balance_before = Currencies::free_balance(DOT, &MONEY_MARKET);

		assert_ok!(EvmAccounts::bind_evm_address(RuntimeOrigin::signed(
			Liquidation::account_id()
		),));
		assert_ok!(EvmAccounts::bind_evm_address(RuntimeOrigin::signed(MONEY_MARKET),));

		// Act
		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(ALICE),
			HDX, // collateral
			DOT, // debt
			bob_evm_address,
			debt_to_cover,
			route,
		));

		// Assert
		// total issuance should not change
		assert_eq!(hdx_total_issuance, Currencies::total_issuance(HDX));
		assert_eq!(dot_total_issuance, Currencies::total_issuance(DOT));

		assert_eq!(hdx_alice_balance_before, Currencies::free_balance(HDX, &ALICE));
		assert_eq!(dot_alice_balance_before, Currencies::free_balance(DOT, &ALICE));

		assert!(Currencies::free_balance(HDX, &Liquidation::account_id()) == 0);
		assert!(Currencies::free_balance(DOT, &Liquidation::account_id()) == 0);

		assert_eq!(Currencies::free_balance(HDX, &TreasuryAccount::get()), 0);
		assert!(Currencies::free_balance(DOT, &TreasuryAccount::get()) > 0);

		assert_eq!(
			Currencies::free_balance(HDX, &MONEY_MARKET),
			hdx_contract_balance_before - 2 * debt_to_cover
		);
		assert_eq!(
			Currencies::free_balance(DOT, &MONEY_MARKET),
			dot_contract_balance_before + debt_to_cover
		);

		expect_last_events(vec![Event::Liquidated {
			user: bob_evm_address,
			debt_asset: DOT,
			collateral_asset: HDX,
			profit: 2_976_143_141_153_081,
		}
		.into()]);
	});
}

#[test]
fn liquidation_should_work_when_debt_and_collateral_asset_is_same() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			EvmAddress::from_slice(&[9; 20])
		));
		let bob_evm_address = EvmAccounts::evm_address(&BOB);
		let debt_to_cover = 1_000 * ONE;

		let hdx_total_issuance = Currencies::total_issuance(HDX);
		let hdx_alice_balance_before = Currencies::free_balance(HDX, &ALICE);
		assert!(Currencies::free_balance(HDX, &Liquidation::account_id()) == 0);
		assert_ok!(EvmAccounts::bind_evm_address(RuntimeOrigin::signed(
			Liquidation::account_id()
		),));
		assert_ok!(EvmAccounts::bind_evm_address(RuntimeOrigin::signed(MONEY_MARKET),));

		// Act
		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(ALICE),
			HDX, // collateral
			HDX, // debt
			bob_evm_address,
			debt_to_cover,
			BoundedVec::new(),
		));

		// Assert
		// total issuance should not change
		assert_eq!(hdx_total_issuance, Currencies::total_issuance(HDX));

		assert_eq!(hdx_alice_balance_before, Currencies::free_balance(HDX, &ALICE));

		assert!(Currencies::free_balance(HDX, &Liquidation::account_id()) == 0);
	});
}

#[test]
fn liquidation_should_fail_if_not_profitable() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			EvmAddress::from_slice(&[9; 20])
		));
		let bob_evm_address = EvmAccounts::evm_address(&BOB);
		let debt_to_cover = 1_000 * ONE;

		let route = Router::get_route(AssetPair {
			asset_in: DOT,
			asset_out: HDX,
		});

		assert_ok!(EvmAccounts::bind_evm_address(RuntimeOrigin::signed(
			Liquidation::account_id()
		),));
		assert_ok!(EvmAccounts::bind_evm_address(RuntimeOrigin::signed(MONEY_MARKET),));

		// Act & Assert
		assert_noop!(
			Liquidation::liquidate(
				RuntimeOrigin::signed(ALICE),
				DOT,
				HDX,
				bob_evm_address,
				debt_to_cover,
				route,
			)
			.map_err(|e| e.error),
			Error::<Test>::NotProfitable
		);
	});
}

#[test]
fn initial_pallet_balance_should_not_change_after_execution() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			EvmAddress::from_slice(&[9; 20])
		));
		let bob_evm_address = EvmAccounts::evm_address(&BOB);
		let debt_to_cover = 1_000 * ONE;
		let initial_pallet_balance = 10_000 * ONE;

		let route = Router::get_route(AssetPair {
			asset_in: HDX,
			asset_out: DOT,
		});

		// set pallet's balance to non-zero value
		assert_ok!(Currencies::deposit(
			HDX,
			&Liquidation::account_id(),
			initial_pallet_balance
		));
		assert_ok!(Currencies::deposit(
			DOT,
			&Liquidation::account_id(),
			initial_pallet_balance
		));

		let hdx_total_issuance = Currencies::total_issuance(HDX);
		let dot_total_issuance = Currencies::total_issuance(DOT);

		let hdx_alice_balance_before = Currencies::free_balance(HDX, &ALICE);
		let dot_alice_balance_before = Currencies::free_balance(DOT, &ALICE);

		let hdx_contract_balance_before = Currencies::free_balance(HDX, &MONEY_MARKET);
		let dot_contract_balance_before = Currencies::free_balance(DOT, &MONEY_MARKET);

		assert_ok!(EvmAccounts::bind_evm_address(RuntimeOrigin::signed(
			Liquidation::account_id()
		),));
		assert_ok!(EvmAccounts::bind_evm_address(RuntimeOrigin::signed(MONEY_MARKET),));

		// Act
		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(ALICE),
			HDX, // collateral
			DOT, // debt
			bob_evm_address,
			debt_to_cover,
			route,
		));

		// Assert
		// total issuance should not change
		assert_eq!(hdx_total_issuance, Currencies::total_issuance(HDX));
		assert_eq!(dot_total_issuance, Currencies::total_issuance(DOT));

		assert_eq!(hdx_alice_balance_before, Currencies::free_balance(HDX, &ALICE));
		assert_eq!(dot_alice_balance_before, Currencies::free_balance(DOT, &ALICE));

		assert!(Currencies::free_balance(DOT, &Liquidation::account_id()) == initial_pallet_balance);
		assert!(Currencies::free_balance(HDX, &Liquidation::account_id()) == initial_pallet_balance);

		assert_eq!(Currencies::free_balance(HDX, &TreasuryAccount::get()), 0);
		assert!(Currencies::free_balance(DOT, &TreasuryAccount::get()) > 0);

		assert_eq!(
			Currencies::free_balance(HDX, &MONEY_MARKET),
			hdx_contract_balance_before - 2 * debt_to_cover
		);
		assert_eq!(
			Currencies::free_balance(DOT, &MONEY_MARKET),
			dot_contract_balance_before + debt_to_cover
		);

		expect_last_events(vec![Event::Liquidated {
			user: bob_evm_address,
			debt_asset: DOT,
			collateral_asset: HDX,
			profit: 2_976_143_141_153_081,
		}
		.into()]);
	});
}

#[test]
fn set_borrowing_contract_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			Liquidation::borrowing_contract(),
			EvmAddress::from_slice(hex_literal::hex!("1b02E051683b5cfaC5929C25E84adb26ECf87B38").as_slice())
		);

		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			EvmAddress::from_slice(&[1; 20])
		));

		assert_eq!(Liquidation::borrowing_contract(), EvmAddress::from_slice(&[1; 20]));
	});
}

#[test]
fn validate_unsigned_should_work_when_submitted_from_local() {
	ExtBuilder::default().build().execute_with(|| {
		let collateral_asset: AssetId = HDX;
		let debt_asset: AssetId = DOT;
		let user: EvmAddress = EvmAccounts::evm_address(&BOB);
		let debt_to_cover: Balance = 1_000;
		let route: Route<AssetId> = BoundedVec::new();

		let c = pallet::Call::<Test>::liquidate {
			collateral_asset,
			debt_asset,
			user,
			debt_to_cover,
			route,
		};

		// Legacy call: base priority, exactly the pre-multi-MM behavior.
		assert_eq!(
			Liquidation::validate_unsigned(TransactionSource::Local, &c),
			Ok(ValidTransaction {
				priority: BASE_UNSIGNED_LIQUIDATION_PRIORITY,
				requires: vec![],
				provides: vec![("liquidate_unsigned", user).encode()],
				longevity: 1,
				propagate: false,
			})
		);
	});
}

#[test]
fn liquidate_with_gigahdx_collateral_should_refuse_when_debt_is_not_hollar() {
	ExtBuilder::default().build().execute_with(|| {
		let bob_evm_address = EvmAccounts::evm_address(&BOB);
		let route = Router::get_route(AssetPair {
			asset_in: HDX,
			asset_out: DOT,
		});
		assert_noop!(
			Liquidation::liquidate(
				RuntimeOrigin::signed(ALICE),
				67,  // GIGAHDX
				DOT, // not HOLLAR
				bob_evm_address,
				1_000 * ONE,
				route,
			)
			.map_err(|e| e.error),
			Error::<Test>::UnsupportedDebtAsset
		);
	});
}

// The priority ladder lives on `liquidate_with_pool` (the worker's call): the supplied
// priority is capped at MAX so a hostile value cannot outrank oracle updates.
#[test]
fn validate_unsigned_priority_should_be_up_to_max_unsigned_liquidation_priority() {
	ExtBuilder::default().build().execute_with(|| {
		let user: EvmAddress = EvmAccounts::evm_address(&BOB);
		let pool = EvmAddress::from_slice(&[9; 20]);

		let c = pallet::Call::<Test>::liquidate_with_pool {
			pool,
			collateral_asset: HDX,
			debt_asset: DOT,
			user,
			debt_to_cover: 1_000,
			route: BoundedVec::new(),
			unsigned_priority: Some(u64::MAX),
		};

		assert_eq!(
			Liquidation::validate_unsigned(TransactionSource::Local, &c),
			Ok(ValidTransaction {
				priority: MAX_UNSIGNED_LIQUIDATION_PRIORITY,
				requires: vec![],
				provides: vec![("liquidate_unsigned", (user, pool)).encode()],
				longevity: 1,
				propagate: false,
			})
		);
	});
}

#[test]
fn liquidate_with_pool_should_liquidate_when_pool_matches_borrowing_contract() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange — identical setup to `liquidation_should_transfer_profit_to_treasury`,
		// so the outcome asserts double as an outcome-parity check against `liquidate`.
		let pool = EvmAddress::from_slice(&[9; 20]);
		assert_ok!(Liquidation::set_borrowing_contract(RuntimeOrigin::root(), pool));
		let bob_evm_address = EvmAccounts::evm_address(&BOB);
		let debt_to_cover = 1_000 * ONE;

		let route = Router::get_route(AssetPair {
			asset_in: HDX,
			asset_out: DOT,
		});

		let hdx_total_issuance = Currencies::total_issuance(HDX);
		let dot_total_issuance = Currencies::total_issuance(DOT);
		let hdx_contract_balance_before = Currencies::free_balance(HDX, &MONEY_MARKET);
		let dot_contract_balance_before = Currencies::free_balance(DOT, &MONEY_MARKET);

		assert_ok!(EvmAccounts::bind_evm_address(RuntimeOrigin::signed(
			Liquidation::account_id()
		),));
		assert_ok!(EvmAccounts::bind_evm_address(RuntimeOrigin::signed(MONEY_MARKET),));

		// Act
		assert_ok!(Liquidation::liquidate_with_pool(
			RuntimeOrigin::none(),
			pool,
			HDX, // collateral
			DOT, // debt
			bob_evm_address,
			debt_to_cover,
			route,
			None,
		));

		// Assert — same outcome as `liquidate` on the identical position
		assert_eq!(hdx_total_issuance, Currencies::total_issuance(HDX));
		assert_eq!(dot_total_issuance, Currencies::total_issuance(DOT));
		assert!(Currencies::free_balance(DOT, &TreasuryAccount::get()) > 0);
		assert_eq!(
			Currencies::free_balance(HDX, &MONEY_MARKET),
			hdx_contract_balance_before - 2 * debt_to_cover
		);
		assert_eq!(
			Currencies::free_balance(DOT, &MONEY_MARKET),
			dot_contract_balance_before + debt_to_cover
		);

		expect_last_events(vec![Event::Liquidated {
			user: bob_evm_address,
			debt_asset: DOT,
			collateral_asset: HDX,
			profit: 2_976_143_141_153_081,
		}
		.into()]);
	});
}

#[test]
fn liquidate_with_pool_should_fail_when_pool_does_not_match_borrowing_contract() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			EvmAddress::from_slice(&[9; 20])
		));
		let bob_evm_address = EvmAccounts::evm_address(&BOB);

		assert_noop!(
			Liquidation::liquidate_with_pool(
				RuntimeOrigin::none(),
				EvmAddress::from_slice(&[8; 20]), // not the borrowing contract
				HDX,
				DOT,
				bob_evm_address,
				1_000 * ONE,
				BoundedVec::new(),
				None,
			)
			.map_err(|e| e.error),
			Error::<Test>::PoolAddressMismatch
		);
	});
}

// The mock's `pool_contract()` returns `None`, so gigahdx-collateral calls must fail
// the pool check with `GigaHdxPoolNotSet` regardless of the provided pool — the gate
// runs before any liquidation logic.
#[test]
fn liquidate_with_pool_should_fail_when_gigahdx_pool_is_not_set() {
	ExtBuilder::default().build().execute_with(|| {
		let bob_evm_address = EvmAccounts::evm_address(&BOB);

		assert_noop!(
			Liquidation::liquidate_with_pool(
				RuntimeOrigin::none(),
				EvmAddress::from_slice(&[9; 20]),
				67,  // GIGAHDX collateral routes the pool check to `pool_contract()`
				222, // HOLLAR
				bob_evm_address,
				1_000 * ONE,
				BoundedVec::new(),
				None,
			)
			.map_err(|e| e.error),
			Error::<Test>::GigaHdxPoolNotSet
		);
	});
}

// `liquidate_with_pool` is the worker-only channel — the public permissionless path is
// `liquidate`. Signed submissions must be rejected before any liquidation logic runs.
#[test]
fn liquidate_with_pool_should_fail_when_origin_is_signed() {
	ExtBuilder::default().build().execute_with(|| {
		let bob_evm_address = EvmAccounts::evm_address(&BOB);

		assert_noop!(
			Liquidation::liquidate_with_pool(
				RuntimeOrigin::signed(ALICE),
				EvmAddress::from_slice(&[9; 20]),
				HDX,
				DOT,
				bob_evm_address,
				1_000 * ONE,
				BoundedVec::new(),
				None,
			)
			.map_err(|e| e.error),
			sp_runtime::traits::BadOrigin
		);
	});
}

#[test]
fn validate_unsigned_should_provide_user_and_pool_when_call_is_liquidate_with_pool() {
	ExtBuilder::default().build().execute_with(|| {
		let pool = EvmAddress::from_slice(&[9; 20]);
		let user: EvmAddress = EvmAccounts::evm_address(&BOB);
		let priority = 1_000;

		let c = pallet::Call::<Test>::liquidate_with_pool {
			pool,
			collateral_asset: HDX,
			debt_asset: DOT,
			user,
			debt_to_cover: 1_000,
			route: BoundedVec::new(),
			unsigned_priority: Some(priority),
		};

		assert_eq!(
			Liquidation::validate_unsigned(TransactionSource::Local, &c),
			Ok(ValidTransaction {
				priority: BASE_UNSIGNED_LIQUIDATION_PRIORITY + priority,
				requires: vec![],
				provides: vec![("liquidate_unsigned", (user, pool)).encode()],
				longevity: 1,
				propagate: false,
			})
		);
	});
}

#[test]
fn validate_unsigned_should_not_collide_when_same_user_is_liquidated_in_two_pools() {
	ExtBuilder::default().build().execute_with(|| {
		let user: EvmAddress = EvmAccounts::evm_address(&BOB);

		let call_for_pool = |pool: EvmAddress| pallet::Call::<Test>::liquidate_with_pool {
			pool,
			collateral_asset: HDX,
			debt_asset: DOT,
			user,
			debt_to_cover: 1_000,
			route: BoundedVec::new(),
			unsigned_priority: None,
		};

		let provides_a = Liquidation::validate_unsigned(
			TransactionSource::Local,
			&call_for_pool(EvmAddress::from_slice(&[9; 20])),
		)
		.expect("valid tx")
		.provides;
		let provides_b = Liquidation::validate_unsigned(
			TransactionSource::Local,
			&call_for_pool(EvmAddress::from_slice(&[8; 20])),
		)
		.expect("valid tx")
		.provides;

		assert_ne!(provides_a, provides_b);
	});
}

// A gigapot shortfall makes `realize_yield` return an error, but that must NOT abort
// the liquidation — folding yield is only an optimisation for the seize snapshot, and an
// underwater position must still be liquidatable. The mock's `realize_yield` always errors;
// the flow must proceed past it and fail at the next step (`snapshot_stake` ->
// `NoGigaHdxPosition`) rather than reverting
// with `RealizeYieldFailed`.
#[test]
fn liquidate_gigahdx_should_proceed_when_realize_yield_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let user_evm = EvmAccounts::evm_address(&BOB);
		// collateral == gigahdx asset (67) routes into `liquidate_gigahdx`;
		// debt == HollarId (222) passes the debt-asset guard.
		assert_noop!(
			Liquidation::liquidate(
				RuntimeOrigin::signed(BOB),
				67,
				222,
				user_evm,
				1_000 * ONE,
				Default::default(),
			)
			.map_err(|e| e.error),
			Error::<Test>::NoGigaHdxPosition
		);
	});
}

fn declared_weight(collateral_asset: AssetId, debt_asset: AssetId) -> Weight {
	Call::<Test>::liquidate {
		collateral_asset,
		debt_asset,
		user: EvmAccounts::evm_address(&BOB),
		debt_to_cover: 1_000 * ONE,
		route: Default::default(),
	}
	.get_dispatch_info()
	.call_weight
}

// One gas budget of the mock runtime: 1_000_000 gas at 1 ref time and 1/4 byte of proof per gas.
fn gas_budgets(count: u64) -> Weight {
	Weight::from_parts(1_000_000, 250_000).saturating_mul(count)
}

fn bind_liquidation_accounts() {
	assert_ok!(Liquidation::set_borrowing_contract(
		RuntimeOrigin::root(),
		EvmAddress::from_slice(&[9; 20])
	));
	assert_ok!(EvmAccounts::bind_evm_address(RuntimeOrigin::signed(
		Liquidation::account_id()
	),));
	assert_ok!(EvmAccounts::bind_evm_address(RuntimeOrigin::signed(MONEY_MARKET),));
}

fn liquidate_hdx_collateral_dot_debt() -> DispatchResultWithPostInfo {
	Liquidation::liquidate(
		RuntimeOrigin::signed(ALICE),
		HDX,
		DOT,
		EvmAccounts::evm_address(&BOB),
		1_000 * ONE,
		Router::get_route(AssetPair {
			asset_in: HDX,
			asset_out: DOT,
		}),
	)
}

#[test]
fn liquidate_should_declare_one_gas_budget_when_debt_is_not_hollar() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			declared_weight(HDX, DOT),
			<() as WeightInfo>::liquidate().saturating_add(gas_budgets(1))
		);
	});
}

#[test]
fn liquidate_should_declare_two_gas_budgets_when_debt_is_hollar() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			declared_weight(DOT, 222),
			<() as WeightInfo>::liquidate().saturating_add(gas_budgets(2))
		);
	});
}

#[test]
fn liquidate_should_declare_four_gas_budgets_when_collateral_is_gigahdx() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			declared_weight(67, 222),
			<() as WeightInfo>::liquidate().saturating_add(gas_budgets(4))
		);
	});
}

#[test]
fn liquidate_should_refund_unused_gas_when_liquidation_succeeds() {
	ExtBuilder::default().build().execute_with(|| {
		bind_liquidation_accounts();
		EvmGasUsed::set(300_000);

		let post_info = liquidate_hdx_collateral_dot_debt().expect("liquidation succeeds");

		assert_eq!(
			post_info.actual_weight,
			Some(declared_weight(HDX, DOT).saturating_sub(Weight::from_parts(700_000, 0)))
		);
	});
}

#[test]
fn liquidate_should_refund_unused_gas_when_liquidation_fails() {
	ExtBuilder::default().build().execute_with(|| {
		bind_liquidation_accounts();
		EvmGasUsed::set(300_000);

		let failure = Liquidation::liquidate(
			RuntimeOrigin::signed(ALICE),
			DOT,
			HDX,
			EvmAccounts::evm_address(&BOB),
			1_000 * ONE,
			Router::get_route(AssetPair {
				asset_in: DOT,
				asset_out: HDX,
			}),
		)
		.expect_err("liquidation is not profitable");

		assert_eq!(failure.error, Error::<Test>::NotProfitable.into());
		assert_eq!(
			failure.post_info.actual_weight,
			Some(declared_weight(DOT, HDX).saturating_sub(Weight::from_parts(700_000, 0)))
		);
	});
}

#[test]
fn liquidate_should_refund_every_gas_budget_when_no_evm_call_is_reached() {
	ExtBuilder::default().build().execute_with(|| {
		// HOLLAR debt with no flash minter configured: fails before the flash loan.
		let failure = Liquidation::liquidate(
			RuntimeOrigin::signed(ALICE),
			DOT,
			222,
			EvmAccounts::evm_address(&BOB),
			1_000 * ONE,
			Default::default(),
		)
		.expect_err("flash minter is not set");

		assert_eq!(failure.error, Error::<Test>::FlashMinterNotSet.into());
		assert_eq!(
			failure.post_info.actual_weight,
			Some(declared_weight(DOT, 222).saturating_sub(Weight::from_parts(2_000_000, 0)))
		);
	});
}

#[test]
fn liquidate_should_charge_whole_gas_budget_when_evm_call_reports_no_gas() {
	ExtBuilder::default().build().execute_with(|| {
		bind_liquidation_accounts();
		EvmGasUsed::set(0);

		let post_info = liquidate_hdx_collateral_dot_debt().expect("liquidation succeeds");

		assert_eq!(post_info.actual_weight, Some(declared_weight(HDX, DOT)));
	});
}

#[test]
fn liquidate_should_not_refund_view_budget_when_collateral_is_gigahdx() {
	ExtBuilder::default().build().execute_with(|| {
		let failure = Liquidation::liquidate(
			RuntimeOrigin::signed(BOB),
			67,
			222,
			EvmAccounts::evm_address(&BOB),
			1_000 * ONE,
			Default::default(),
		)
		.expect_err("mock has no gigahdx position");

		assert_eq!(
			failure.post_info.actual_weight,
			Some(declared_weight(67, 222).saturating_sub(Weight::from_parts(3_000_000, 0)))
		);
	});
}

#[test]
fn liquidate_should_charge_recorded_proof_size_when_proof_is_recorded() {
	ExtBuilder::default().build().execute_with(|| {
		bind_liquidation_accounts();
		EvmGasUsed::set(300_000);
		RecordedProofSize::set(Some(10_000));
		EvmProofSize::set(40_000);

		let post_info = liquidate_hdx_collateral_dot_debt().expect("liquidation succeeds");

		assert_eq!(
			post_info.actual_weight,
			Some(Weight::from_parts(
				declared_weight(HDX, DOT).ref_time() - 700_000,
				40_000
			))
		);
	});
}

#[test]
fn liquidate_should_charge_declared_proof_size_when_recorded_proof_exceeds_it() {
	ExtBuilder::default().build().execute_with(|| {
		bind_liquidation_accounts();
		EvmGasUsed::set(300_000);
		RecordedProofSize::set(Some(0));
		EvmProofSize::set(10_000_000);

		let post_info = liquidate_hdx_collateral_dot_debt().expect("liquidation succeeds");

		assert_eq!(
			post_info.actual_weight,
			Some(declared_weight(HDX, DOT).saturating_sub(Weight::from_parts(700_000, 0)))
		);
	});
}

const POOL: [u8; 20] = [9; 20];

fn submit_liquidation_call(valid_until: u64) -> pallet::Call<Test> {
	pallet::Call::<Test>::submit_liquidation {
		pool: EvmAddress::from_slice(&POOL),
		collateral_asset: HDX,
		debt_asset: DOT,
		user: EvmAccounts::evm_address(&BOB),
		debt_to_cover: 1_000,
		route: BoundedVec::new(),
		priority: 1_000,
		valid_until,
	}
}

#[test]
fn validate_unsigned_should_accept_liquidation_when_valid_until_is_including_block() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(10);

		assert_eq!(
			Liquidation::validate_unsigned(TransactionSource::Local, &submit_liquidation_call(10)),
			Ok(ValidTransaction {
				priority: BASE_UNSIGNED_LIQUIDATION_PRIORITY + 1_000,
				requires: vec![],
				provides: vec![(
					"liquidate_unsigned",
					(EvmAccounts::evm_address(&BOB), EvmAddress::from_slice(&POOL))
				)
					.encode()],
				longevity: 1,
				propagate: false,
			})
		);
	});
}

#[test]
fn validate_unsigned_should_keep_liquidation_in_pool_until_valid_until_when_it_is_blocks_ahead() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(10);

		let validity = Liquidation::validate_unsigned(TransactionSource::Local, &submit_liquidation_call(11));

		assert_eq!(validity.map(|v| v.longevity), Ok(2));
	});
}

#[test]
fn validate_unsigned_should_return_stale_when_valid_until_has_passed() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(11);

		assert_eq!(
			Liquidation::validate_unsigned(TransactionSource::Local, &submit_liquidation_call(10)),
			Err(InvalidTransaction::Stale.into())
		);
	});
}

#[test]
fn validate_unsigned_should_accept_liquidation_when_valid_until_is_last_block_of_max_lifetime() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(10);

		assert_ok!(Liquidation::validate_unsigned(
			TransactionSource::Local,
			&submit_liquidation_call(10 + u64::from(MAX_UNSIGNED_LIQUIDATION_LIFETIME) - 1)
		));
	});
}

#[test]
fn validate_unsigned_should_reject_liquidation_when_valid_until_exceeds_max_lifetime() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(10);

		assert_eq!(
			Liquidation::validate_unsigned(
				TransactionSource::Local,
				&submit_liquidation_call(10 + u64::from(MAX_UNSIGNED_LIQUIDATION_LIFETIME))
			),
			Err(InvalidTransaction::Call.into())
		);
	});
}

#[test]
fn validate_unsigned_should_reject_expiring_liquidation_when_source_is_external() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(10);

		assert_eq!(
			Liquidation::validate_unsigned(TransactionSource::External, &submit_liquidation_call(10)),
			Err(InvalidTransaction::Call.into())
		);
	});
}

#[test]
fn pre_dispatch_should_return_stale_when_valid_until_has_passed() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(11);

		assert_eq!(
			Liquidation::pre_dispatch(&submit_liquidation_call(10)),
			Err(InvalidTransaction::Stale.into())
		);
	});
}

#[test]
fn submit_liquidation_should_liquidate_when_pool_matches_borrowing_contract() {
	ExtBuilder::default().build().execute_with(|| {
		let pool = EvmAddress::from_slice(&POOL);
		assert_ok!(Liquidation::set_borrowing_contract(RuntimeOrigin::root(), pool));
		bind_liquidation_accounts();
		let bob_evm_address = EvmAccounts::evm_address(&BOB);
		let route = Router::get_route(AssetPair {
			asset_in: HDX,
			asset_out: DOT,
		});

		assert_ok!(Liquidation::submit_liquidation(
			RuntimeOrigin::none(),
			pool,
			HDX,
			DOT,
			bob_evm_address,
			1_000 * ONE,
			route,
			1_000,
			1,
		));

		expect_last_events(vec![Event::Liquidated {
			user: bob_evm_address,
			debt_asset: DOT,
			collateral_asset: HDX,
			profit: 2_976_143_141_153_081,
		}
		.into()]);
	});
}

#[test]
fn submit_liquidation_should_fail_when_pool_does_not_match_borrowing_contract() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			EvmAddress::from_slice(&POOL)
		));

		assert_noop!(
			Liquidation::submit_liquidation(
				RuntimeOrigin::none(),
				EvmAddress::from_slice(&[7; 20]),
				HDX,
				DOT,
				EvmAccounts::evm_address(&BOB),
				1_000 * ONE,
				BoundedVec::new(),
				1_000,
				1,
			)
			.map_err(|e| e.error),
			Error::<Test>::PoolAddressMismatch
		);
	});
}

#[test]
fn submit_liquidation_should_fail_when_origin_is_signed() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Liquidation::submit_liquidation(
				RuntimeOrigin::signed(ALICE),
				EvmAddress::from_slice(&POOL),
				HDX,
				DOT,
				EvmAccounts::evm_address(&BOB),
				1_000 * ONE,
				BoundedVec::new(),
				1_000,
				1,
			)
			.map_err(|e| e.error),
			sp_runtime::traits::BadOrigin
		);
	});
}
