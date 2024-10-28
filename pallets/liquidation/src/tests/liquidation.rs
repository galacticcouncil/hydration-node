// we don't need to run tests with benchmarking feature
#![cfg(not(feature = "runtime-benchmarks"))]
#![allow(clippy::bool_assert_comparison)]

pub use crate::tests::mock::*;
use frame_support::assert_ok;
use orml_traits::MultiCurrency;
use hydradx_traits::{
	evm::InspectEvmAccounts,
	router::{AssetPair, RouteProvider},
};
use crate::Event;

pub fn expect_last_events(e: Vec<RuntimeEvent>) {
	test_utils::expect_events::<RuntimeEvent, Test>(e);
}

#[test]
fn liquidation_should_transfer_profit_to_treasury() {
	ExtBuilder::default().build().execute_with(|| {
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

		assert_ok!(
			EvmAccounts::bind_evm_address(
				RuntimeOrigin::signed(Liquidation::account_id()),
			)
		);
		assert_ok!(
			EvmAccounts::bind_evm_address(
				RuntimeOrigin::signed(MONEY_MARKET),
			)
		);

		assert_ok!(Liquidation::liquidate(
			RuntimeOrigin::signed(ALICE),
			HDX, // collateral
			DOT, // debt
			bob_evm_address,
			debt_to_cover,
			route,
		));

		// total issuance should not change
		assert_eq!(hdx_total_issuance, Currencies::total_issuance(HDX));
		assert_eq!(dot_total_issuance, Currencies::total_issuance(DOT));

		assert_eq!(hdx_alice_balance_before, Currencies::free_balance(HDX, &ALICE));
		assert_eq!(dot_alice_balance_before, Currencies::free_balance(DOT, &ALICE));

		assert!(Currencies::free_balance(HDX, &Liquidation::account_id()) == 0);
		assert!(Currencies::free_balance(DOT, &Liquidation::account_id()) == 0);

		assert_eq!(Currencies::free_balance(HDX, &TreasuryAccount::get()), 0);
		let profit = 2976143141153081;
		assert_eq!(Currencies::free_balance(DOT, &TreasuryAccount::get()), profit);

		assert_eq!(Currencies::free_balance(HDX, &MONEY_MARKET), hdx_contract_balance_before - 2 * debt_to_cover);
		assert_eq!(Currencies::free_balance(DOT, &MONEY_MARKET), dot_contract_balance_before + debt_to_cover);

		expect_last_events(vec![Event::Liquidated {
			liquidator: ALICE,
			evm_address: bob_evm_address,
			debt_asset: DOT,
			collateral_asset: HDX,
			debt_to_cover,
		}
		.into()]);
	});
}
