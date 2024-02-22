#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{
	assert_ok,
	dispatch::DispatchInfo,
	sp_runtime::{traits::SignedExtension, FixedU128, Permill},
	weights::Weight,
};
use frame_system::RawOrigin;
use hydradx_runtime::{
	Balances, Currencies, EmaOracle, MultiTransactionPayment, Omnipool, Router, RuntimeOrigin, Tokens,
};
use orml_traits::currency::MultiCurrency;
use primitives::Price;

use hydradx_adapters::OraclePriceProvider;
use hydradx_traits::{
	pools::SpotPriceProvider,
	router::{AssetPair, RouteProvider},
	OraclePeriod, PriceOracle,
};
use xcm_emulator::TestExt;

#[test]
fn non_native_fee_payment_works_with_oracle_price_based_on_onchain_route() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: BTC },
		);

		let info = DispatchInfo {
			weight: Weight::from_parts(106_957_000, 0),
			..Default::default()
		};
		let len: usize = 10;

		assert_ok!(
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0).pre_dispatch(
				&AccountId::from(BOB),
				&call,
				&info,
				len,
			)
		);
		let bob_balance = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));
		assert_eq!(bob_balance, 999_962);

		assert_ok!(hydradx_runtime::Balances::force_set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			2_000_000_000_000 * UNITS,
		));

		init_omnipool();

		hydradx_run_to_block(4);

		let dave_balance = hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(DAVE));
		assert_eq!(dave_balance, 1_000_000_000_000_000_000_000);

		let call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: DAI },
		);

		assert_ok!(
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0).pre_dispatch(
				&AccountId::from(DAVE),
				&call,
				&info,
				len,
			)
		);

		let dave_balance = hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(DAVE));
		assert_eq!(dave_balance, 999_992_364_637_822_103_500); //Price based on oracle with onchain route
	});
}

const HITCHHIKER: [u8; 32] = [42u8; 32];

#[test]
fn fee_currency_on_account_lifecycle() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_eq!(MultiTransactionPayment::get_currency(AccountId::from(HITCHHIKER)), None);

		// ------------ set on create ------------
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(BOB.into()),
			HITCHHIKER.into(),
			1,
			50_000_000_000_000,
		));

		assert_eq!(
			Tokens::free_balance(1, &AccountId::from(HITCHHIKER)),
			50_000_000_000_000
		);
		assert_eq!(
			MultiTransactionPayment::get_currency(AccountId::from(HITCHHIKER)),
			Some(1)
		);

		// ------------ remove on delete ------------
		assert_ok!(Tokens::transfer_all(
			RuntimeOrigin::signed(HITCHHIKER.into()),
			BOB.into(),
			1,
			false,
		));

		assert_eq!(MultiTransactionPayment::get_currency(AccountId::from(HITCHHIKER)), None);
	});
}

#[test]
fn pepe_is_not_registered() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(MultiTransactionPayment::add_currency(
			RuntimeOrigin::root(),
			PEPE,
			Price::from(10)
		));
	});
}

#[test]
fn fee_currency_cannot_be_set_to_not_accepted_asset() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// assemble
		let amount = 50_000_000 * UNITS;
		assert_eq!(MultiTransactionPayment::get_currency(AccountId::from(HITCHHIKER)), None);

		// act
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(BOB.into()),
			HITCHHIKER.into(),
			PEPE,
			amount,
		));

		// assert
		assert_eq!(Tokens::free_balance(PEPE, &AccountId::from(HITCHHIKER)), amount);
		assert_eq!(MultiTransactionPayment::get_currency(AccountId::from(HITCHHIKER)), None);
	});
}

#[test]
fn fee_currency_should_not_change_when_account_holds_native_currency_already() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			HITCHHIKER.into(),
			UNITS,
		));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			HITCHHIKER.into(),
			1,
			50_000_000_000_000,
		));

		assert_eq!(Balances::free_balance(AccountId::from(HITCHHIKER)), UNITS);
		assert_eq!(MultiTransactionPayment::get_currency(AccountId::from(HITCHHIKER)), None);
	});
}

#[test]
fn fee_currency_should_not_change_when_account_holds_other_token_already() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			HITCHHIKER.into(),
			1,
			50_000_000_000_000,
		));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			HITCHHIKER.into(),
			2,
			50_000_000_000,
		));

		assert_eq!(
			MultiTransactionPayment::get_currency(AccountId::from(HITCHHIKER)),
			Some(1)
		);
	});
}

#[test]
fn fee_currency_should_reset_to_default_when_account_spends_tokens() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			HITCHHIKER.into(),
			1,
			50_000_000_000_000,
		));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			HITCHHIKER.into(),
			2,
			50_000_000_000,
		));
		assert_ok!(Tokens::transfer_all(
			RuntimeOrigin::signed(HITCHHIKER.into()),
			ALICE.into(),
			1,
			false,
		));

		assert_eq!(MultiTransactionPayment::get_currency(AccountId::from(HITCHHIKER)), None);
	});
}

#[test]
fn omnipool_spotprice_and_onchain_price_should_be_very_similar() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();

		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			Omnipool::protocol_account(),
			DOT,
			3000 * UNITS as i128,
		));

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			FixedU128::from_inner(25_650_000_000_000_000),
			Permill::from_percent(1),
			AccountId::from(BOB),
		));
		do_trade_to_populate_oracle(DAI, DOT, 10 * UNITS);

		set_relaychain_block_number(10);

		//Act
		let spot_price = Omnipool::spot_price(DAI, DOT).unwrap();

		let default_route = Router::get_route(AssetPair::new(DAI, DOT));
		let onchain_oracle_price = OraclePriceProvider::<AssetId, EmaOracle, hydradx_runtime::LRNA>::price(
			&default_route,
			OraclePeriod::Short,
		)
		.unwrap();

		let onchain_oracle_price = FixedU128::from_rational(onchain_oracle_price.n, onchain_oracle_price.d);

		//Assert
		assert_eq!(spot_price.to_float(), onchain_oracle_price.to_float());
	});
}

fn do_trade_to_populate_oracle(asset_1: AssetId, asset_2: AssetId, amount: Balance) {
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		CHARLIE.into(),
		LRNA,
		1000000000000 * UNITS,
		0,
	));

	assert_ok!(Omnipool::sell(
		RuntimeOrigin::signed(CHARLIE.into()),
		LRNA,
		asset_1,
		amount,
		Balance::MIN
	));

	assert_ok!(Omnipool::sell(
		RuntimeOrigin::signed(CHARLIE.into()),
		LRNA,
		asset_2,
		amount,
		Balance::MIN
	));
}
