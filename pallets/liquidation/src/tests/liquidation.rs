// we don't need to run tests with benchmarking feature
#![cfg(not(feature = "runtime-benchmarks"))]
#![allow(clippy::bool_assert_comparison)]

pub use crate::tests::mock::*;
use crate::{Error, Event};
use frame_support::{assert_noop, assert_ok};
use hydradx_traits::{
	evm::InspectEvmAccounts,
	router::{AssetPair, RouteProvider},
};
use orml_traits::parameters::sp_runtime::BoundedVec;
use orml_traits::MultiCurrency;

pub fn expect_last_events(e: Vec<RuntimeEvent>) {
	test_utils::expect_events::<RuntimeEvent, Test>(e);
}
use hydradx_traits::evm::EvmAddress;

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
			debt_to_cover,
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
			),
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
			debt_to_cover,
			profit: 2_976_143_141_153_081,
		}
		.into()]);
	});
}
