#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::dispatch::{DispatchInfo, Weight};
use frame_support::{
	assert_ok,
	traits::{OnFinalize, OnInitialize},
};
use frame_system::RawOrigin;

use hydradx_adapters::OraclePriceProvider;
use hydradx_runtime::EmaOracle;
use hydradx_runtime::Omnipool;
use hydradx_runtime::Router;
use hydradx_runtime::{Balances, Currencies, MultiTransactionPayment, RuntimeOrigin, Tokens};
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::AssetPair;
use hydradx_traits::router::RouteProvider;
use hydradx_traits::OraclePeriod;
use hydradx_traits::PriceOracle;
use orml_traits::currency::MultiCurrency;
use polkadot_primitives::v2::BlockNumber;
use primitives::Price;
use sp_runtime::traits::SignedExtension;
use sp_runtime::FixedU128;
use sp_runtime::Permill;
use xcm_emulator::TestExt;

pub fn hydra_run_to_block(to: BlockNumber) {
	while hydradx_runtime::System::block_number() < to {
		let b = hydradx_runtime::System::block_number();

		hydradx_runtime::System::on_finalize(b);
		hydradx_runtime::MultiTransactionPayment::on_finalize(b);

		hydradx_runtime::System::on_initialize(b + 1);
		hydradx_runtime::MultiTransactionPayment::on_initialize(b + 1);

		hydradx_runtime::System::set_block_number(b + 1);
	}
}

#[test]
fn non_native_fee_payment_works_with_oracle_price_based_on_onchain_route() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: BTC },
		);

		let info = DispatchInfo {
			weight: Weight::from_ref_time(106_957_000),
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
		assert_eq!(bob_balance, 999_959);

		assert_ok!(hydradx_runtime::Balances::set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			2_000_000_000_000 * UNITS,
			0,
		));

		init_omnipool();

		hydra_run_to_block(2);

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
		assert_eq!(dave_balance, 999_999_999_692_871_594_551); //Price based on oracle with onchain route
	});
}

const HITCHHIKER: [u8; 32] = [42u8; 32];

#[test]
fn fee_currency_on_account_lifecycle() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_eq!(
			MultiTransactionPayment::account_currency(&AccountId::from(HITCHHIKER)),
			0,
		);

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
			MultiTransactionPayment::account_currency(&AccountId::from(HITCHHIKER)),
			1
		);

		// ------------ remove on delete ------------
		assert_ok!(Tokens::transfer_all(
			RuntimeOrigin::signed(HITCHHIKER.into()),
			BOB.into(),
			1,
			false,
		));

		assert_eq!(
			MultiTransactionPayment::account_currency(&AccountId::from(HITCHHIKER)),
			0,
		);
	});
}

#[test]
fn pepe_is_not_registered() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(MultiTransactionPayment::add_currency(RuntimeOrigin::root(), PEPE,));
	});
}

#[test]
fn fee_currency_cannot_be_set_to_not_accepted_asset() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// assemble
		let amount = 50_000_000 * UNITS;
		assert_eq!(
			MultiTransactionPayment::account_currency(&AccountId::from(HITCHHIKER)),
			0,
		);

		// act
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(BOB.into()),
			HITCHHIKER.into(),
			PEPE,
			amount,
		));

		// assert
		assert_eq!(Tokens::free_balance(PEPE, &AccountId::from(HITCHHIKER)), amount);
		assert_eq!(
			MultiTransactionPayment::account_currency(&AccountId::from(HITCHHIKER)),
			0,
		);
	});
}

#[test]
fn fee_currency_should_not_change_when_account_holds_native_currency_already() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_ok!(Balances::set_balance(
			RuntimeOrigin::root(),
			HITCHHIKER.into(),
			UNITS,
			0,
		));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			HITCHHIKER.into(),
			1,
			50_000_000_000_000,
		));

		assert_eq!(Balances::free_balance(&AccountId::from(HITCHHIKER)), UNITS);
		assert_eq!(
			MultiTransactionPayment::account_currency(&AccountId::from(HITCHHIKER)),
			0,
		);
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
			MultiTransactionPayment::account_currency(&AccountId::from(HITCHHIKER)),
			1
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

		assert_eq!(
			MultiTransactionPayment::account_currency(&AccountId::from(HITCHHIKER)),
			0,
		);
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
		let spot_price =
			pallet_omnipool::provider::OmnipoolSpotPriceProvider::<hydradx_runtime::Runtime>::get_price(DAI, DOT)
				.unwrap();

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
