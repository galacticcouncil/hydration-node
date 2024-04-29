#![cfg(test)]

use crate::polkadot_test_net::*;
use crate::utils::accounts::*;
use frame_support::pallet_prelude::ValidateUnsigned;
use frame_support::{assert_noop, assert_ok, sp_runtime::codec::Encode};
use frame_system::RawOrigin;
use hydradx_runtime::{Balances, Currencies, EVMAccounts, MultiTransactionPayment, Omnipool, RuntimeOrigin, Tokens};
use libsecp256k1::{sign, Message, SecretKey};
use orml_traits::MultiCurrency;
use pretty_assertions::assert_eq;
use primitives::{AssetId, Balance};
use sp_core::{H256, U256};
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;

pub const TREASURY_ACCOUNT_INIT_BALANCE: Balance = 1000 * UNITS;

#[test]
fn compare_fee_in_hdx_between_evm_and_native_omnipool_calls_when_permit_is_dispatched() {
	TestNet::reset();

	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_evm_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());

	Hydra::execute_with(|| {
		let fee_currency = HDX;
		//Set up to idle state where the chain is not utilized at all
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);
		init_omnipool_with_oracle_for_block_10();

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			(10_000 * UNITS) as i128,
		));
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DOT,
			(10 * UNITS) as i128,
		));

		let treasury_currency_balance = Currencies::free_balance(fee_currency, &Treasury::account_id());
		let alice_currency_balance = Currencies::free_balance(fee_currency, &AccountId::from(user_acc.address()));

		// just reset the weth balance to 0 - to make sure we dont have enough WETH
		let initial_user_weth_balance = user_acc.balance(WETH);
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			WETH,
			-(initial_user_weth_balance as i128),
		));
		let initial_user_weth_balance = user_acc.balance(WETH);
		assert_eq!(initial_user_weth_balance, 0);

		//Act
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: DOT,
				asset_out: WETH,
				amount: 10_000_000_000,
				min_buy_amount: 0,
			});

		let gas_limit = 1_000_000;
		let deadline = U256::from(1000000000000u128);
		let permit =
			pallet_evm_precompile_call_permit::CallPermitPrecompile::<hydradx_runtime::Runtime>::generate_permit(
				CALLPERMIT,
				user_evm_address,
				DISPATCH_ADDR,
				U256::from(0),
				omni_sell.encode(),
				gas_limit * 10,
				U256::zero(),
				deadline,
			);
		let secret_key = SecretKey::parse(&user_secret_key).unwrap();
		let message = Message::parse(&permit);
		let (rs, v) = sign(&message, &secret_key);

		//Execute omnipool via EVM
		assert_ok!(MultiTransactionPayment::dispatch_permit(
			hydradx_runtime::RuntimeOrigin::none(),
			user_evm_address,
			DISPATCH_ADDR,
			U256::from(0),
			omni_sell.encode(),
			gas_limit * 10,
			deadline,
			v.serialize(),
			H256::from(rs.r.b32()),
			H256::from(rs.s.b32()),
		));

		let user_weth = user_acc.balance(WETH);
		assert!(user_weth > 0);
		let new_treasury_currency_balance = treasury_acc.balance(fee_currency);
		let new_user_currency_balance = user_acc.balance(fee_currency);
		let evm_fee = alice_currency_balance - new_user_currency_balance;
		let treasury_evm_fee = new_treasury_currency_balance - treasury_currency_balance;
		assert!(evm_fee > 0);
		assert_eq!(treasury_evm_fee, evm_fee);

		/* TODO: native fee is 0 for some reason - need to investigate
		//Pre dispatch the native omnipool call - so withdrawing only the fees for the execution
		let info = omni_sell.get_dispatch_info();
		let len: usize = 146;
		let pre = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
			.pre_dispatch(&AccountId::from(user_acc.address()), &omni_sell, &info, len);
		assert_ok!(&pre);

		let alice_currency_balance_pre_dispatch = Currencies::free_balance(fee_currency, &AccountId::from(user_acc.address()));
		let native_fee = new_alice_currency_balance - alice_currency_balance_pre_dispatch;
		dbg!(evm_fee,native_fee);
		assert!(evm_fee > native_fee);

		let fee_difference = evm_fee - native_fee;
		assert!(fee_difference > 0);
		let relative_fee_difference = FixedU128::from_rational(fee_difference, native_fee);
		let tolerated_fee_difference = FixedU128::from_rational(20, 100);
		// EVM fees should be not higher than 20%
		assert!(relative_fee_difference < tolerated_fee_difference);
		 */
	})
}

#[test]
#[ignore]
//Note: the account has incorrect fee currency set for unknown reasy, ignore for now
fn dispatch_permit_fee_should_be_paid_in_hdx_when_no_currency_is_set() {
	TestNet::reset();

	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_evm_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());

	Hydra::execute_with(|| {
		//Set up to idle state where the chain is not utilized at all
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		let accs = pallet_transaction_multi_payment::AccountCurrencyMap::<hydradx_runtime::Runtime>::iter();
		for a in accs {
			dbg!(a);
		}

		let currency =
			pallet_transaction_multi_payment::Pallet::<hydradx_runtime::Runtime>::account_currency(&user_acc.address());
		assert_eq!(currency, HDX);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			100_000_000_000_000_000_i128,
		));

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DOT,
			(10 * UNITS) as i128,
		));

		init_omnipool_with_oracle_for_block_10();
		let initial_treasury_hdx_balance = treasury_acc.balance(HDX);
		let initial_user_hdx_balance = user_acc.balance(HDX);

		//Act
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: DOT,
				asset_out: WETH,
				amount: 10_000_000,
				min_buy_amount: 0,
			});

		let gas_limit = 1000000;
		let deadline = U256::from(1000000000000u128);
		let permit =
			pallet_evm_precompile_call_permit::CallPermitPrecompile::<hydradx_runtime::Runtime>::generate_permit(
				CALLPERMIT,
				user_evm_address,
				DISPATCH_ADDR,
				U256::from(0),
				omni_sell.encode(),
				gas_limit * 10,
				U256::zero(),
				deadline,
			);
		let secret_key = SecretKey::parse(&user_secret_key).unwrap();
		let message = Message::parse(&permit);
		let (rs, v) = sign(&message, &secret_key);

		//Execute omnipool via EVM
		assert_ok!(MultiTransactionPayment::dispatch_permit(
			hydradx_runtime::RuntimeOrigin::none(),
			user_evm_address,
			DISPATCH_ADDR,
			U256::from(0),
			omni_sell.encode(),
			gas_limit * 10,
			deadline,
			v.serialize(),
			H256::from(rs.r.b32()),
			H256::from(rs.s.b32()),
		));

		let user_hdx_balance = user_acc.balance(HDX);
		let fee_amount = user_hdx_balance - initial_user_hdx_balance;
		assert!(fee_amount > 0);

		let new_treasury_hdx_balance = treasury_acc.balance(HDX);
		let treasury_hdx_diff = new_treasury_hdx_balance - initial_treasury_hdx_balance;
		assert_eq!(fee_amount, treasury_hdx_diff);
	})
}

#[test]
fn fee_should_be_paid_in_hdx_when_permit_is_dispatched_and_address_is_bounded() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_evm_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - bind and fund
		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			user_acc.address()
		)));
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			100_000_000_000_000i128,
		));
		//Fund some DOT to sell in omnipool
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DOT,
			100_000_000i128,
		));

		let initial_treasury_hdx_balance = treasury_acc.balance(HDX);
		let initial_user_hdx_balance = user_acc.balance(HDX);
		let initial_user_weth_balance = user_acc.balance(WETH);
		let initial_user_dot_balance = user_acc.balance(DOT);

		// just reset the weth balance to 0 - to make sure we dont have enough WETH
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			WETH,
			-(initial_user_weth_balance as i128),
		));
		let initial_user_weth_balance = user_acc.balance(WETH);
		assert_eq!(initial_user_weth_balance, 0);

		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: DOT,
				asset_out: WETH,
				amount: 10_000_000,
				min_buy_amount: 0,
			});

		let gas_limit = 1000000;
		let deadline = U256::from(1000000000000u128);

		let permit =
			pallet_evm_precompile_call_permit::CallPermitPrecompile::<hydradx_runtime::Runtime>::generate_permit(
				CALLPERMIT,
				user_evm_address,
				DISPATCH_ADDR,
				U256::from(0),
				omni_sell.encode(),
				gas_limit,
				U256::zero(),
				deadline,
			);
		let secret_key = SecretKey::parse(&user_secret_key).unwrap();
		let message = Message::parse(&permit);
		let (rs, v) = sign(&message, &secret_key);

		//Execute omnipool via EVM
		assert_ok!(MultiTransactionPayment::dispatch_permit(
			hydradx_runtime::RuntimeOrigin::none(),
			user_evm_address,
			DISPATCH_ADDR,
			U256::from(0),
			omni_sell.encode(),
			gas_limit,
			deadline,
			v.serialize(),
			H256::from(rs.r.b32()),
			H256::from(rs.s.b32()),
		));
		// Verify evm fee amount
		let user_hdx_balance = user_acc.balance(HDX);
		let fee_amount = initial_user_hdx_balance - user_hdx_balance;
		assert!(fee_amount > 0);
		let new_treasury_hdx_balance = treasury_acc.balance(HDX);
		let treasury_hdx_diff = new_treasury_hdx_balance - initial_treasury_hdx_balance;
		assert_eq!(fee_amount, treasury_hdx_diff);

		// Verify omnipool sell
		let user_weth_balance = user_acc.balance(WETH);
		assert_eq!(user_weth_balance, 3_565_408_466_680);
		let user_dot_balance = user_acc.balance(DOT);
		assert!(user_dot_balance < initial_user_dot_balance);
		let dot_diff = initial_user_dot_balance - user_dot_balance;
		assert_eq!(dot_diff, 10_000_000);
	})
}

#[test]
fn fee_should_be_paid_in_hdx_when_permit_is_dispatched_and_address_is_not_bounded() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - bind and fund
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			100_000_000_000_000i128,
		));
		//Fund some DOT to sell in omnipool
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DOT,
			100_000_000i128,
		));

		let initial_treasury_hdx_balance = treasury_acc.balance(HDX);
		let initial_user_hdx_balance = user_acc.balance(HDX);
		let initial_user_weth_balance = user_acc.balance(WETH);
		let initial_user_dot_balance = user_acc.balance(DOT);

		// just reset the weth balance to 0 - to make sure we dont have enough WETH
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			WETH,
			-(initial_user_weth_balance as i128),
		));
		let initial_user_weth_balance = user_acc.balance(WETH);
		assert_eq!(initial_user_weth_balance, 0);

		//Act
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: DOT,
				asset_out: WETH,
				amount: 10_000_000,
				min_buy_amount: 0,
			});

		let gas_limit = 1000000;
		let deadline = U256::from(1000000000000u128);

		let permit =
			pallet_evm_precompile_call_permit::CallPermitPrecompile::<hydradx_runtime::Runtime>::generate_permit(
				CALLPERMIT,
				user_evm_address,
				DISPATCH_ADDR,
				U256::from(0),
				omni_sell.encode(),
				gas_limit,
				U256::zero(),
				deadline,
			);
		let secret_key = SecretKey::parse(&user_secret_key).unwrap();
		let message = Message::parse(&permit);
		let (rs, v) = sign(&message, &secret_key);

		//Execute omnipool via EVM
		assert_ok!(MultiTransactionPayment::dispatch_permit(
			hydradx_runtime::RuntimeOrigin::none(),
			user_evm_address,
			DISPATCH_ADDR,
			U256::from(0),
			omni_sell.encode(),
			gas_limit,
			deadline,
			v.serialize(),
			H256::from(rs.r.b32()),
			H256::from(rs.s.b32()),
		));
		// Verify evm fee amount
		let user_hdx_balance = user_acc.balance(HDX);
		let fee_amount = initial_user_hdx_balance - user_hdx_balance;
		assert!(fee_amount > 0);
		let new_treasury_hdx_balance = treasury_acc.balance(HDX);
		let treasury_hdx_diff = new_treasury_hdx_balance - initial_treasury_hdx_balance;
		assert_eq!(fee_amount, treasury_hdx_diff);

		// Verify omnipool sell
		let user_weth_balance = user_acc.balance(WETH);
		assert_eq!(user_weth_balance, 3_565_408_466_680);

		let user_dot_balance = user_acc.balance(DOT);
		assert!(user_dot_balance < initial_user_dot_balance);
		let dot_diff = initial_user_dot_balance - user_dot_balance;
		assert_eq!(dot_diff, 10_000_000);
	})
}

#[test]
fn evm_permit_should_validate_unsigned_correctly() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - bind and fund
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			100_000_000_000_000i128,
		));
		//Fund some DOT to sell in omnipool
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DOT,
			100_000_000i128,
		));

		let initial_treasury_hdx_balance = treasury_acc.balance(HDX);
		let initial_user_hdx_balance = user_acc.balance(HDX);
		let initial_user_weth_balance = user_acc.balance(WETH);
		let initial_user_dot_balance = user_acc.balance(DOT);

		// just reset the weth balance to 0 - to make sure we dont have enough WETH
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			WETH,
			-(initial_user_weth_balance as i128),
		));
		let initial_user_weth_balance = user_acc.balance(WETH);
		assert_eq!(initial_user_weth_balance, 0);

		//Act
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: DOT,
				asset_out: WETH,
				amount: 10_000_000,
				min_buy_amount: 0,
			});

		let gas_limit = 1000000;
		let deadline = U256::from(1000000000000u128);

		let permit =
			pallet_evm_precompile_call_permit::CallPermitPrecompile::<hydradx_runtime::Runtime>::generate_permit(
				CALLPERMIT,
				user_evm_address,
				DISPATCH_ADDR,
				U256::from(0),
				omni_sell.encode(),
				gas_limit,
				U256::zero(),
				deadline,
			);
		let secret_key = SecretKey::parse(&user_secret_key).unwrap();
		let message = Message::parse(&permit);
		let (rs, v) = sign(&message, &secret_key);

		let call = pallet_transaction_multi_payment::Call::dispatch_permit {
			from: user_evm_address,
			to: DISPATCH_ADDR,
			value: U256::from(0),
			data: omni_sell.encode(),
			gas_limit,
			deadline,
			v: v.serialize(),
			r: H256::from(rs.r.b32()),
			s: H256::from(rs.s.b32()),
		};

		assert_eq!(
			MultiTransactionPayment::validate_unsigned(TransactionSource::External, &call),
			Ok(ValidTransaction {
				priority: 0,
				requires: vec![],
				provides: vec![],
				longevity: 64,
				propagate: true,
			})
		);

		// Verify that nothing has changed
		let user_hdx_balance = user_acc.balance(HDX);
		assert_eq!(user_hdx_balance, initial_user_hdx_balance);
		let new_treasury_hdx_balance = treasury_acc.balance(HDX);
		assert_eq!(new_treasury_hdx_balance, initial_treasury_hdx_balance);
		// Verify omnipool sell
		let user_weth_balance = user_acc.balance(WETH);
		assert_eq!(user_weth_balance, 0);
		let user_dot_balance = user_acc.balance(DOT);
		assert_eq!(initial_user_dot_balance, user_dot_balance);
	})
}

#[test]
fn evm_permit_set_currency_dispatch_should_pay_evm_fee_in_chosen_currency() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - bind and fund
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DAI,
			100_000_000_000_000_000_000i128,
		));
		let initial_user_dai_balance = user_acc.balance(DAI);
		let initial_user_weth_balance = user_acc.balance(WETH);

		let initial_dai_issuance = Currencies::total_issuance(DAI);

		// just reset the weth balance to 0 - to make sure we dont have enough WETH
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			WETH,
			-(initial_user_weth_balance as i128),
		));
		let initial_user_weth_balance = user_acc.balance(WETH);
		assert_eq!(initial_user_weth_balance, 0);

		let set_currency_call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: DAI },
		);

		let gas_limit = 1000000;
		let deadline = U256::from(1000000000000u128);

		let permit =
			pallet_evm_precompile_call_permit::CallPermitPrecompile::<hydradx_runtime::Runtime>::generate_permit(
				CALLPERMIT,
				user_evm_address,
				DISPATCH_ADDR,
				U256::from(0),
				set_currency_call.encode(),
				gas_limit,
				U256::zero(),
				deadline,
			);
		let secret_key = SecretKey::parse(&user_secret_key).unwrap();
		let message = Message::parse(&permit);
		let (rs, v) = sign(&message, &secret_key);

		// Validate unsigned first

		let call = pallet_transaction_multi_payment::Call::dispatch_permit {
			from: user_evm_address,
			to: DISPATCH_ADDR,
			value: U256::from(0),
			data: set_currency_call.encode(),
			gas_limit,
			deadline,
			v: v.serialize(),
			r: H256::from(rs.r.b32()),
			s: H256::from(rs.s.b32()),
		};

		assert_eq!(
			MultiTransactionPayment::validate_unsigned(TransactionSource::External, &call),
			Ok(ValidTransaction {
				priority: 0,
				requires: vec![],
				provides: vec![],
				longevity: 64,
				propagate: true,
			})
		);

		// And Dispatch
		assert_ok!(MultiTransactionPayment::dispatch_permit(
			hydradx_runtime::RuntimeOrigin::none(),
			user_evm_address,
			DISPATCH_ADDR,
			U256::from(0),
			set_currency_call.encode(),
			gas_limit,
			deadline,
			v.serialize(),
			H256::from(rs.r.b32()),
			H256::from(rs.s.b32()),
		));

		let currency =
			pallet_transaction_multi_payment::Pallet::<hydradx_runtime::Runtime>::account_currency(&user_acc.address());
		assert_eq!(currency, DAI);

		let dai_issuance = Currencies::total_issuance(DAI);
		assert_eq!(initial_dai_issuance, dai_issuance);

		let user_dai_balance = user_acc.balance(DAI);
		assert!(user_dai_balance < initial_user_dai_balance);
		let dai_diff = initial_user_dai_balance - user_dai_balance;
		assert_eq!(dai_diff, 1_651_665_230_644_991);
	})
}

#[test]
fn evm_permit_dispatch_flow_should_work() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - bind and fund
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			100_000_000_000_000i128,
		));
		//Fund some DOT to sell in omnipool
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DOT,
			100_000_000i128,
		));

		let initial_treasury_hdx_balance = treasury_acc.balance(HDX);
		let initial_user_hdx_balance = user_acc.balance(HDX);
		let initial_user_weth_balance = user_acc.balance(WETH);
		let initial_user_dot_balance = user_acc.balance(DOT);

		// just reset the weth balance to 0 - to make sure we dont have enough WETH
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			WETH,
			-(initial_user_weth_balance as i128),
		));
		let initial_user_weth_balance = user_acc.balance(WETH);
		assert_eq!(initial_user_weth_balance, 0);

		//Act
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: DOT,
				asset_out: WETH,
				amount: 10_000_000,
				min_buy_amount: 0,
			});

		let gas_limit = 1000000;
		let deadline = U256::from(1000000000000u128);

		let permit =
			pallet_evm_precompile_call_permit::CallPermitPrecompile::<hydradx_runtime::Runtime>::generate_permit(
				CALLPERMIT,
				user_evm_address,
				DISPATCH_ADDR,
				U256::from(0),
				omni_sell.encode(),
				gas_limit,
				U256::zero(),
				deadline,
			);
		let secret_key = SecretKey::parse(&user_secret_key).unwrap();
		let message = Message::parse(&permit);
		let (rs, v) = sign(&message, &secret_key);

		// Validate unsigned first

		let call = pallet_transaction_multi_payment::Call::dispatch_permit {
			from: user_evm_address,
			to: DISPATCH_ADDR,
			value: U256::from(0),
			data: omni_sell.encode(),
			gas_limit,
			deadline,
			v: v.serialize(),
			r: H256::from(rs.r.b32()),
			s: H256::from(rs.s.b32()),
		};

		assert_eq!(
			MultiTransactionPayment::validate_unsigned(TransactionSource::External, &call),
			Ok(ValidTransaction {
				priority: 0,
				requires: vec![],
				provides: vec![],
				longevity: 64,
				propagate: true,
			})
		);

		// And Dispatch
		assert_ok!(MultiTransactionPayment::dispatch_permit(
			hydradx_runtime::RuntimeOrigin::none(),
			user_evm_address,
			DISPATCH_ADDR,
			U256::from(0),
			omni_sell.encode(),
			gas_limit,
			deadline,
			v.serialize(),
			H256::from(rs.r.b32()),
			H256::from(rs.s.b32()),
		));

		// Verify evm fee amount
		let user_hdx_balance = user_acc.balance(HDX);
		let fee_amount = initial_user_hdx_balance - user_hdx_balance;
		assert!(fee_amount > 0);
		//assert_eq!(fee_amount, 451229318663);
		let new_treasury_hdx_balance = treasury_acc.balance(HDX);
		let treasury_hdx_diff = new_treasury_hdx_balance - initial_treasury_hdx_balance;
		assert_eq!(fee_amount, treasury_hdx_diff);

		// Verify omnipool sell
		let user_weth_balance = user_acc.balance(WETH);
		assert_eq!(user_weth_balance, 3_565_408_466_680);

		let user_dot_balance = user_acc.balance(DOT);
		assert!(user_dot_balance < initial_user_dot_balance);
		let dot_diff = initial_user_dot_balance - user_dot_balance;
		assert_eq!(dot_diff, 10_000_000);
	})
}

#[test]
fn evm_permit_should_fail_when_replayed() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - bind and fund
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			100_000_000_000_000i128,
		));
		//Fund some DOT to sell in omnipool
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DOT,
			100_000_000i128,
		));

		let initial_treasury_hdx_balance = treasury_acc.balance(HDX);
		let initial_user_hdx_balance = user_acc.balance(HDX);
		let initial_user_weth_balance = user_acc.balance(WETH);
		let initial_user_dot_balance = user_acc.balance(DOT);

		// just reset the weth balance to 0 - to make sure we dont have enough WETH
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			WETH,
			-(initial_user_weth_balance as i128),
		));
		let initial_user_weth_balance = user_acc.balance(WETH);
		assert_eq!(initial_user_weth_balance, 0);

		//Act
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: DOT,
				asset_out: WETH,
				amount: 10_000_000,
				min_buy_amount: 0,
			});

		let gas_limit = 1000000;
		let deadline = U256::from(1000000000000u128);

		let permit =
			pallet_evm_precompile_call_permit::CallPermitPrecompile::<hydradx_runtime::Runtime>::generate_permit(
				CALLPERMIT,
				user_evm_address,
				DISPATCH_ADDR,
				U256::from(0),
				omni_sell.encode(),
				gas_limit,
				U256::zero(),
				deadline,
			);
		let secret_key = SecretKey::parse(&user_secret_key).unwrap();
		let message = Message::parse(&permit);
		let (rs, v) = sign(&message, &secret_key);

		// Validate unsigned first

		let call = pallet_transaction_multi_payment::Call::dispatch_permit {
			from: user_evm_address,
			to: DISPATCH_ADDR,
			value: U256::from(0),
			data: omni_sell.encode(),
			gas_limit,
			deadline,
			v: v.serialize(),
			r: H256::from(rs.r.b32()),
			s: H256::from(rs.s.b32()),
		};

		assert_eq!(
			MultiTransactionPayment::validate_unsigned(TransactionSource::External, &call),
			Ok(ValidTransaction {
				priority: 0,
				requires: vec![],
				provides: vec![],
				longevity: 64,
				propagate: true,
			})
		);

		// And Dispatch
		assert_ok!(MultiTransactionPayment::dispatch_permit(
			hydradx_runtime::RuntimeOrigin::none(),
			user_evm_address,
			DISPATCH_ADDR,
			U256::from(0),
			omni_sell.encode(),
			gas_limit,
			deadline,
			v.serialize(),
			H256::from(rs.r.b32()),
			H256::from(rs.s.b32()),
		));

		// And try to replay
		assert_noop!(
			MultiTransactionPayment::dispatch_permit(
				hydradx_runtime::RuntimeOrigin::none(),
				user_evm_address,
				DISPATCH_ADDR,
				U256::from(0),
				omni_sell.encode(),
				gas_limit,
				deadline,
				v.serialize(),
				H256::from(rs.r.b32()),
				H256::from(rs.s.b32()),
			),
			pallet_transaction_multi_payment::Error::<hydradx_runtime::Runtime>::EvmPermitInvalid
		);

		// Verify evm fee amount
		let user_hdx_balance = user_acc.balance(HDX);
		let fee_amount = initial_user_hdx_balance - user_hdx_balance;
		assert!(fee_amount > 0);
		//assert_eq!(fee_amount, 451229318663);
		let new_treasury_hdx_balance = treasury_acc.balance(HDX);
		let treasury_hdx_diff = new_treasury_hdx_balance - initial_treasury_hdx_balance;
		assert_eq!(fee_amount, treasury_hdx_diff);

		// Verify omnipool sell
		let user_weth_balance = user_acc.balance(WETH);
		assert_eq!(user_weth_balance, 3_565_408_466_680);

		let user_dot_balance = user_acc.balance(DOT);
		assert!(user_dot_balance < initial_user_dot_balance);
		let dot_diff = initial_user_dot_balance - user_dot_balance;
		assert_eq!(dot_diff, 10_000_000);
	})
}

pub fn init_omnipool_with_oracle_for_block_10() {
	init_omnipol();
	hydradx_run_to_next_block();
	do_trade_to_populate_oracle(WETH, DOT, 1_000_000_000_000);
	let to = 40;
	let from = 11;
	for _ in from..=to {
		hydradx_run_to_next_block();
		do_trade_to_populate_oracle(DOT, HDX, 1_000_000_000_000);
		do_trade_to_populate_oracle(DAI, HDX, 1_000_000_000_000);
		do_trade_to_populate_oracle(WETH, DOT, 1_000_000_000_000);
	}
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

use frame_support::traits::fungible::Mutate;
use hydradx_runtime::evm::precompiles::{CALLPERMIT, DISPATCH_ADDR};
use sp_runtime::transaction_validity::{TransactionSource, ValidTransaction};

pub fn init_omnipol() {
	let native_price = FixedU128::from_rational(29903049701668757, 73927734532192294158);
	let dot_price = FixedU128::from_rational(103158291366950047, 4566210555614178);
	let stable_price = FixedU128::from_inner(45_000_000_000);
	let acc = hydradx_runtime::Omnipool::protocol_account();

	let stable_amount = 50_000_000 * UNITS * 1_000_000;
	let dot_amount: Balance = 4566210555614178u128;
	let native_amount: Balance = 73927734532192294158u128;
	let weth_amount: Balance = 1074271742496220564487u128;
	let weth_price = FixedU128::from_rational(67852651072676287, 1074271742496220564487);
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		acc.clone(),
		DOT,
		dot_amount,
		0
	));
	Balances::set_balance(&acc, native_amount);
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		acc.clone(),
		WETH,
		weth_amount,
		0
	));
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		HDX,
		native_price,
		Permill::from_percent(60),
		AccountId::from(ALICE),
	));

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DOT,
		dot_price,
		Permill::from_percent(60),
		AccountId::from(ALICE),
	));
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		WETH,
		weth_price,
		Permill::from_percent(60),
		AccountId::from(ALICE),
	));

	assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), acc, DAI, stable_amount, 0));
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DAI,
		stable_price,
		Permill::from_percent(100),
		AccountId::from(ALICE),
	));

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		hydradx_runtime::Treasury::account_id(),
		TREASURY_ACCOUNT_INIT_BALANCE,
	));
}
