#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydradx_runtime::{Omnipool, RuntimeOrigin};
use orml_traits::MultiCurrency;
use xcm_emulator::TestExt;

#[test]
fn sell_h2o_for_asset_should_route_to_hdx_pool() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		init_omnipool();

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			LRNA,
			1000 * UNITS as i128,
		));

		let sell_amount = 100 * UNITS;

		let initial_alice_lrna = hydradx_runtime::Tokens::free_balance(LRNA, &AccountId::from(ALICE));
		let initial_alice_dai = hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(ALICE));
		let initial_hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		let initial_dai_state = Omnipool::load_asset_state(DAI).unwrap();

		// Act
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			LRNA,
			DAI,
			sell_amount,
			0
		));

		let final_alice_lrna = hydradx_runtime::Tokens::free_balance(LRNA, &AccountId::from(ALICE));
		let final_alice_dai = hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(ALICE));
		let final_hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		let final_dai_state = Omnipool::load_asset_state(DAI).unwrap();

		// Assert
		assert_eq!(
			initial_alice_lrna - final_alice_lrna,
			sell_amount,
			"ALICE should have spent sell_amount of LRNA"
		);
		assert!(
			final_alice_dai > initial_alice_dai,
			"ALICE should have received DAI"
		);

		assert!(
			final_hdx_state.hub_reserve > initial_hdx_state.hub_reserve,
			"HDX hub_reserve should increase when H2O is routed to HDX subpool"
		);

		let dai_received = final_alice_dai - initial_alice_dai;
		assert_eq!(
			initial_dai_state.reserve - final_dai_state.reserve,
			dai_received,
			"DAI reserve decrease should equal tokens sent to user"
		);

		expect_hydra_events(vec![pallet_omnipool::Event::Rerouted {
			from: DAI,
			to: CORE_ASSET_ID,
			hub_amount: sell_amount,
		}
		.into()]);
	});
}

#[test]
fn sell_h2o_for_hdx_should_emit_rerouted_event() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		init_omnipool();

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			LRNA,
			1000 * UNITS as i128,
		));

		let sell_amount = 100 * UNITS;

		let initial_alice_lrna = hydradx_runtime::Tokens::free_balance(LRNA, &AccountId::from(ALICE));
		let initial_alice_hdx = hydradx_runtime::Balances::free_balance(&AccountId::from(ALICE));
		let initial_hdx_state = Omnipool::load_asset_state(HDX).unwrap();

		// Act
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			LRNA,
			CORE_ASSET_ID,
			sell_amount,
			0
		));

		let final_alice_lrna = hydradx_runtime::Tokens::free_balance(LRNA, &AccountId::from(ALICE));
		let final_alice_hdx = hydradx_runtime::Balances::free_balance(&AccountId::from(ALICE));
		let final_hdx_state = Omnipool::load_asset_state(HDX).unwrap();

		// Assert
		assert_eq!(
			initial_alice_lrna - final_alice_lrna,
			sell_amount,
			"ALICE should have spent sell_amount of LRNA"
		);
		assert!(
			final_alice_hdx > initial_alice_hdx,
			"ALICE should have received HDX"
		);

		assert!(
			final_hdx_state.hub_reserve > initial_hdx_state.hub_reserve,
			"HDX hub_reserve should increase when H2O is routed to HDX subpool"
		);

		let hdx_received = final_alice_hdx - initial_alice_hdx;
		assert_eq!(
			initial_hdx_state.reserve - final_hdx_state.reserve,
			hdx_received,
			"HDX reserve decrease should equal tokens sent to user"
		);

		expect_hydra_events(vec![pallet_omnipool::Event::Rerouted {
			from: CORE_ASSET_ID,
			to: CORE_ASSET_ID,
			hub_amount: sell_amount,
		}
		.into()]);
	});
}
