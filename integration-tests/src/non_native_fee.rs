#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::sp_runtime::codec::Encode;
use frame_support::{
	assert_ok,
	dispatch::{DispatchInfo, GetDispatchInfo},
	sp_runtime::{traits::DispatchTransaction, FixedU128, Permill},
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
	evm::InspectEvmAccounts,
	pools::SpotPriceProvider,
	router::{AssetPair, RouteProvider},
	OraclePeriod, PriceOracle,
};
use test_utils::assert_eq_approx;
use xcm_emulator::TestExt;

#[test]
fn non_native_fee_payment_works_with_oracle_price_based_on_onchain_route() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Ensure AcceptedCurrencyPrice is populated (transient storage set by on_initialize)
		use frame_support::traits::OnInitialize;
		hydradx_runtime::MultiTransactionPayment::on_initialize(1);

		let call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: BTC },
		);

		let info = DispatchInfo {
			call_weight: Weight::from_parts(106_957_000, 0),
			..Default::default()
		};
		let len: usize = 10;

		assert_ok!(
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
				.validate_and_prepare(Some(AccountId::from(BOB)).into(), &call, &info, len, 0,)
		);
		let bob_balance = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));
		assert!(bob_balance > 0);

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
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
				.validate_and_prepare(Some(AccountId::from(DAVE)).into(), &call, &info, len, 0,)
		);

		let dave_new_balance = hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(DAVE));
		assert!(dave_balance - dave_new_balance > 0);
	});
}

#[test]
fn set_currency_should_work_in_batch_transaction_when_first_tx() {
	TestNet::reset();

	// batch
	Hydra::execute_with(|| {
		// Ensure AcceptedCurrencyPrice is populated (transient storage set by on_initialize)
		use frame_support::traits::OnInitialize;
		hydradx_runtime::MultiTransactionPayment::on_initialize(1);

		let first_inner_call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: BTC },
		);
		let second_inner_call = hydradx_runtime::RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
		let call = hydradx_runtime::RuntimeCall::Utility(pallet_utility::Call::batch {
			calls: vec![first_inner_call, second_inner_call],
		});

		let info = DispatchInfo {
			call_weight: Weight::from_parts(106_957_000, 0),
			..Default::default()
		};
		let len: usize = 10;

		assert_ok!(
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
				.validate_and_prepare(Some(AccountId::from(BOB)).into(), &call, &info, len, 0,)
		);
		let bob_balance = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));
		assert!(bob_balance > 0);
	});

	TestNet::reset();

	// batch_all
	Hydra::execute_with(|| {
		// Ensure AcceptedCurrencyPrice is populated (transient storage set by on_initialize)
		use frame_support::traits::OnInitialize;
		hydradx_runtime::MultiTransactionPayment::on_initialize(1);

		let first_inner_call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: BTC },
		);
		let second_inner_call = hydradx_runtime::RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
		let call = hydradx_runtime::RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![first_inner_call, second_inner_call],
		});

		let info = DispatchInfo {
			call_weight: Weight::from_parts(106_957_000, 0),
			..Default::default()
		};
		let len: usize = 10;

		assert_ok!(
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
				.validate_and_prepare(Some(AccountId::from(BOB)).into(), &call, &info, len, 0,)
		);
		let bob_balance = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));
		assert!(bob_balance > 0);
	});

	TestNet::reset();

	// force_batch
	Hydra::execute_with(|| {
		// Ensure AcceptedCurrencyPrice is populated (transient storage set by on_initialize)
		use frame_support::traits::OnInitialize;
		hydradx_runtime::MultiTransactionPayment::on_initialize(1);

		let first_inner_call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: BTC },
		);
		let second_inner_call = hydradx_runtime::RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
		let call = hydradx_runtime::RuntimeCall::Utility(pallet_utility::Call::force_batch {
			calls: vec![first_inner_call, second_inner_call],
		});

		let info = DispatchInfo {
			call_weight: Weight::from_parts(106_957_000, 0),
			..Default::default()
		};
		let len: usize = 10;

		assert_ok!(
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
				.validate_and_prepare(Some(AccountId::from(BOB)).into(), &call, &info, len, 0,)
		);
		let bob_balance = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));
		assert!(bob_balance > 0);
	});
}

#[test]
fn set_currency_should_work_in_dispatch_with_extra_gas() {
	// Regression test for https://github.com/galacticcouncil/hydration-node/issues/1296
	// dispatch_with_extra_gas wrapping set_currency should charge the fee in the NEW currency,
	// not in the previously stored account currency.
	TestNet::reset();

	Hydra::execute_with(|| {
		use frame_support::traits::OnInitialize;
		hydradx_runtime::MultiTransactionPayment::on_initialize(1);

		// BOB starts with no explicit fee currency (defaults to HDX).
		assert_eq!(MultiTransactionPayment::get_currency(AccountId::from(BOB)), None);

		let set_currency_call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: BTC },
		);

		let call = hydradx_runtime::RuntimeCall::Dispatcher(pallet_dispatcher::Call::dispatch_with_extra_gas {
			call: Box::new(set_currency_call),
			extra_gas: 0,
		});

		let info = call.get_dispatch_info();
		let len = call.encoded_size();

		let bob_hdx_before = hydradx_runtime::Balances::free_balance(AccountId::from(BOB));
		let bob_btc_before = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));

		assert_ok!(
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
				.validate_and_prepare(Some(AccountId::from(BOB)).into(), &call, &info, len, 0,)
		);

		let bob_hdx_after = hydradx_runtime::Balances::free_balance(AccountId::from(BOB));
		let bob_btc_after = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));

		// Fee must be charged in BTC (the new currency declared inside dispatch_with_extra_gas).
		assert!(
			bob_btc_after < bob_btc_before,
			"BTC balance should decrease — fee must be charged in the new currency"
		);
		// HDX must not be touched.
		assert_eq!(bob_hdx_after, bob_hdx_before, "HDX must not be charged");
	});
}

#[test]
fn set_currency_should_work_in_dispatch_with_extra_gas_for_evm_account() {
	// Regression test for https://github.com/galacticcouncil/hydration-node/issues/1293
	// EVM accounts calling dispatch_with_extra_gas { set_currency } should charge the fee
	// in the NEW currency, not in WETH (the default EVM account fee currency).
	TestNet::reset();

	Hydra::execute_with(|| {
		use frame_support::traits::OnInitialize;
		hydradx_runtime::MultiTransactionPayment::on_initialize(1);

		let evm_acc = evm_account();

		// Fund the EVM account with WETH (default EVM fee currency) and DAI
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			evm_acc.clone(),
			WETH,
			1_000_000_000_000_000_000i128,
		));
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			evm_acc.clone(),
			DAI,
			1_000_000_000_000_000_000i128,
		));

		// EVM accounts have WETH automatically set as their fee currency on account creation
		assert_eq!(
			MultiTransactionPayment::get_currency(evm_acc.clone()),
			Some(WETH),
			"EVM account should have WETH set as default fee currency"
		);

		let set_currency_call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: DAI },
		);

		let call = hydradx_runtime::RuntimeCall::Dispatcher(pallet_dispatcher::Call::dispatch_with_extra_gas {
			call: Box::new(set_currency_call),
			extra_gas: 0,
		});

		let info = call.get_dispatch_info();
		let len = call.encoded_size();

		let weth_before = hydradx_runtime::Tokens::free_balance(WETH, &evm_acc);
		let dai_before = hydradx_runtime::Tokens::free_balance(DAI, &evm_acc);

		assert_ok!(
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
				.validate_and_prepare(Some(evm_acc.clone()).into(), &call, &info, len, 0,)
		);

		let weth_after = hydradx_runtime::Tokens::free_balance(WETH, &evm_acc);
		let dai_after = hydradx_runtime::Tokens::free_balance(DAI, &evm_acc);

		// Fee must be charged in DAI (the new currency), not WETH (EVM default).
		assert!(
			dai_after < dai_before,
			"DAI balance should decrease — fee must be charged in the new currency"
		);
		// WETH must not be touched.
		assert_eq!(weth_after, weth_before, "WETH must not be charged");
	});
}

#[test]
fn set_currency_should_not_work_in_dispatch_with_extra_gas_when_not_direct_inner_call() {
	// set_currency must only be recognised when it is the direct inner call of
	// dispatch_with_extra_gas, not when it is nested deeper (e.g. inside a batch inside the
	// dispatcher). In that case the fee should fall back to the previously stored currency.
	TestNet::reset();

	Hydra::execute_with(|| {
		use frame_support::traits::OnInitialize;
		hydradx_runtime::MultiTransactionPayment::on_initialize(1);

		let set_currency_call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: BTC },
		);

		// Wrap set_currency inside a batch, then wrap that inside dispatch_with_extra_gas.
		// The resolver only looks one level deep, so BTC should NOT be picked up.
		let batch = hydradx_runtime::RuntimeCall::Utility(pallet_utility::Call::batch {
			calls: vec![set_currency_call],
		});

		let call = hydradx_runtime::RuntimeCall::Dispatcher(pallet_dispatcher::Call::dispatch_with_extra_gas {
			call: Box::new(batch),
			extra_gas: 0,
		});

		let info = call.get_dispatch_info();
		let len = call.encoded_size();

		let bob_btc_before = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));

		assert_ok!(
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
				.validate_and_prepare(Some(AccountId::from(BOB)).into(), &call, &info, len, 0,)
		);

		let bob_btc_after = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));

		// BTC must not be charged — the resolver did not reach the nested set_currency.
		assert_eq!(
			bob_btc_after, bob_btc_before,
			"BTC must not be charged when set_currency is nested deeper than the direct inner call"
		);
	});
}

#[test]
fn set_currency_should_not_work_in_batch_transaction_when_not_first_tx() {
	TestNet::reset();

	// batch
	Hydra::execute_with(|| {
		// Ensure AcceptedCurrencyPrice is populated (transient storage set by on_initialize)
		use frame_support::traits::OnInitialize;
		hydradx_runtime::MultiTransactionPayment::on_initialize(1);

		let first_inner_call = hydradx_runtime::RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
		let second_inner_call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: BTC },
		);
		let call = hydradx_runtime::RuntimeCall::Utility(pallet_utility::Call::batch {
			calls: vec![first_inner_call, second_inner_call],
		});

		let info = DispatchInfo {
			call_weight: Weight::from_parts(106_957_000, 0),
			..Default::default()
		};
		let len: usize = 10;

		let bob_initial_balance = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));

		assert_ok!(
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
				.validate_and_prepare(Some(AccountId::from(BOB)).into(), &call, &info, len, 0,)
		);
		let bob_balance = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));
		assert_eq!(bob_balance, bob_initial_balance);
	});

	TestNet::reset();

	// batch_all
	Hydra::execute_with(|| {
		// Ensure AcceptedCurrencyPrice is populated (transient storage set by on_initialize)
		use frame_support::traits::OnInitialize;
		hydradx_runtime::MultiTransactionPayment::on_initialize(1);

		let first_inner_call = hydradx_runtime::RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
		let second_inner_call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: BTC },
		);
		let call = hydradx_runtime::RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![first_inner_call, second_inner_call],
		});

		let info = DispatchInfo {
			call_weight: Weight::from_parts(106_957_000, 0),
			..Default::default()
		};
		let len: usize = 10;

		let bob_initial_balance = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));

		assert_ok!(
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
				.validate_and_prepare(Some(AccountId::from(BOB)).into(), &call, &info, len, 0,)
		);
		let bob_balance = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));
		assert_eq!(bob_balance, bob_initial_balance);
	});

	TestNet::reset();

	// force_batch
	Hydra::execute_with(|| {
		// Ensure AcceptedCurrencyPrice is populated (transient storage set by on_initialize)
		use frame_support::traits::OnInitialize;
		hydradx_runtime::MultiTransactionPayment::on_initialize(1);

		let first_inner_call = hydradx_runtime::RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
		let second_inner_call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: BTC },
		);
		let call = hydradx_runtime::RuntimeCall::Utility(pallet_utility::Call::force_batch {
			calls: vec![first_inner_call, second_inner_call],
		});

		let info = DispatchInfo {
			call_weight: Weight::from_parts(106_957_000, 0),
			..Default::default()
		};
		let len: usize = 10;

		let bob_initial_balance = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));

		assert_ok!(
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
				.validate_and_prepare(Some(AccountId::from(BOB)).into(), &call, &info, len, 0,)
		);
		let bob_balance = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));
		assert_eq!(bob_balance, bob_initial_balance);
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
fn fee_currency_on_evm_account_lifecycle() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_eq!(MultiTransactionPayment::get_currency(AccountId::from(HITCHHIKER)), None);

		let evm_address = hydradx_runtime::EVMAccounts::evm_address(&Into::<AccountId>::into(HITCHHIKER));
		let truncated_account: AccountId = hydradx_runtime::EVMAccounts::truncated_account_id(evm_address);

		// ------------ set on create ------------
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(BOB.into()),
			truncated_account.clone(),
			DAI,
			50_000_000_000_000,
		));

		assert_eq!(Tokens::free_balance(DAI, &truncated_account), 50_000_000_000_000);
		assert_eq!(
			MultiTransactionPayment::get_currency(truncated_account.clone()),
			Some(DAI)
		);

		// ------------ remove on delete ------------
		assert_ok!(Tokens::transfer_all(
			RuntimeOrigin::signed(truncated_account.clone()),
			BOB.into(),
			DAI,
			false,
		));

		assert_eq!(MultiTransactionPayment::get_currency(truncated_account), None);
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

		go_to_block(10);

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
		assert_eq_approx!(
			spot_price.to_float(),
			onchain_oracle_price.to_float(),
			0.0001,
			"too different"
		);
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
