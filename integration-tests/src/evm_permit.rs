#![cfg(test)]

use crate::polkadot_test_net::*;
use crate::utils::accounts::*;
use frame_support::dispatch::GetDispatchInfo;
use frame_support::pallet_prelude::ValidateUnsigned;
use frame_support::storage::with_transaction;
use frame_support::traits::fungible::Mutate;
use frame_support::traits::Contains;
use frame_support::{assert_noop, assert_ok, sp_runtime::codec::Encode};
use frame_system::RawOrigin;
use hydradx_adapters::price::ConvertBalance;
use hydradx_runtime::evm::precompiles::{CALLPERMIT, DISPATCH_ADDR};
use hydradx_runtime::types::ShortOraclePrice;
use hydradx_runtime::AssetRegistry;
use hydradx_runtime::DOT_ASSET_LOCATION;
use hydradx_runtime::XYK;
use hydradx_runtime::{
	Balances, Currencies, DotAssetId, MultiTransactionPayment, Omnipool, RuntimeCall, RuntimeOrigin, Tokens,
	XykPaymentAssetSupport,
};
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use hydradx_traits::Mutate as AssetRegistryMutate;
use libsecp256k1::{sign, Message, SecretKey};
use orml_traits::MultiCurrency;
use pallet_evm_accounts::EvmNonceProvider;
use pallet_transaction_multi_payment::EVMPermit;
use pretty_assertions::assert_eq;
use primitives::constants::currency::UNITS;
use primitives::{AssetId, Balance, Index};
use sp_core::{H256, U256};
use sp_runtime::traits::{Convert, Zero};
use sp_runtime::traits::SignedExtension;
use sp_runtime::transaction_validity::InvalidTransaction;
use sp_runtime::transaction_validity::TransactionValidityError;
use sp_runtime::transaction_validity::{TransactionSource, ValidTransaction};
use sp_runtime::DispatchResult;
use sp_runtime::TransactionOutcome;
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
	let fee_currency = WETH;

	Hydra::execute_with(|| {
		assert_eq!(
			MultiTransactionPayment::account_currency(&user_acc.address()),
			fee_currency
		);

		init_omnipool_with_oracle_for_block_10();

		// Fee asset
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			user_acc.address(),
			WETH,
			to_ether(1),
			0,
		));

		// Asset in
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			(10 * UNITS) as i128,
		));

		let treasury_currency_balance = Currencies::free_balance(fee_currency, &Treasury::account_id());
		let alice_currency_balance = Currencies::free_balance(fee_currency, &AccountId::from(user_acc.address()));

		// just reset the weth balance to 0 - to make sure we don't have enough DAI
		let initial_user_dai_balance = user_acc.balance(DAI);
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DAI,
			-(initial_user_dai_balance as i128),
		));
		let initial_user_dai_balance = user_acc.balance(DAI);
		assert_eq!(initial_user_dai_balance, 0);

		//Act
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
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

		assert!(user_acc.balance(DAI) > 0); // Omnipool sell passed
		let new_treasury_currency_balance = treasury_acc.balance(fee_currency);
		let new_user_currency_balance = user_acc.balance(fee_currency);
		let evm_fee = alice_currency_balance - new_user_currency_balance;
		let treasury_evm_fee = new_treasury_currency_balance - treasury_currency_balance;
		assert!(evm_fee > 0);
		assert_eq!(treasury_evm_fee, evm_fee);

		// Pre dispatch the native omnipool call - so withdrawing only the fees for the execution
		let info = omni_sell.get_dispatch_info();
		let len: usize = 146;
		let pre = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
			.pre_dispatch(&AccountId::from(user_acc.address()), &omni_sell, &info, len);
		assert_ok!(&pre);

		let alice_currency_balance_pre_dispatch =
			Currencies::free_balance(fee_currency, &AccountId::from(user_acc.address()));
		let native_fee = new_user_currency_balance - alice_currency_balance_pre_dispatch;
		assert!(evm_fee > native_fee);

		let fee_difference = evm_fee - native_fee;
		assert!(fee_difference > 0);
		let relative_fee_difference = FixedU128::from_rational(fee_difference, native_fee);
		let tolerated_fee_difference = FixedU128::from_rational(30, 100);
		// EVM fees should be not higher than 20%
		assert!(
			relative_fee_difference < tolerated_fee_difference,
			"relative_fee_difference: {:?} is bigger than tolerated {:?}",
			relative_fee_difference,
			tolerated_fee_difference
		);
	})
}

#[test]
fn dispatch_permit_fee_should_be_paid_in_weth_when_no_currency_is_set() {
	TestNet::reset();

	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_evm_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());
	let fee_currency = WETH;

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
		assert_eq!(currency, fee_currency);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			(10 * UNITS) as i128,
		));

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			user_acc.address(),
			fee_currency,
			to_ether(1), // Works as fee_currency is WETH
			0,
		));

		init_omnipool_with_oracle_for_block_10();
		let initial_treasury_fee_balance = treasury_acc.balance(fee_currency);
		let initial_user_fee_currency_balance = user_acc.balance(fee_currency);
		let initial_user_hdx_balance = user_acc.balance(HDX);
		let initial_user_dai_balance = user_acc.balance(DAI);

		//Act
		let sell_amount = 10_000_000;
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: sell_amount,
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

		// Assert omnipool sell passed
		assert_ne!(initial_user_dai_balance, user_acc.balance(DAI));
		assert_eq!(initial_user_hdx_balance - user_acc.balance(HDX), sell_amount);

		// Assert fees
		let fee_amount = initial_user_fee_currency_balance - user_acc.balance(fee_currency);
		assert!(fee_amount > 0);

		let treasury_fee_diff = treasury_acc.balance(fee_currency) - initial_treasury_fee_balance;
		assert_eq!(fee_amount, treasury_fee_diff);
	})
}

#[test]
fn fee_should_be_paid_in_weth_when_permit_is_dispatched_and_address_is_not_bounded() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());
	let fee_currency = WETH;

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - bind and fund
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			user_acc.address(),
			fee_currency,
			to_ether(1),
			0,
		));

		//Fund some HDX to sell in omnipool
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			(10 * UNITS) as i128,
		));

		let initial_treasury_fee_balance = treasury_acc.balance(fee_currency);
		let initial_user_fee_currency_balance = user_acc.balance(fee_currency);
		let initial_user_hdx_balance = user_acc.balance(HDX);
		let initial_user_dai_balance = user_acc.balance(DAI);

		// just reset the weth balance to 0 - to make sure we don't have enough DAI
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DAI,
			-(initial_user_dai_balance as i128),
		));
		let initial_user_dai_balance = user_acc.balance(DAI);
		assert_eq!(initial_user_dai_balance, 0);

		//Act
		let sell_amount = 10_000_000;
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: sell_amount,
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
		let fee_amount = initial_user_fee_currency_balance - user_acc.balance(fee_currency);
		assert!(fee_amount > 0);
		let treasury_fee_diff = treasury_acc.balance(WETH) - initial_treasury_fee_balance;
		assert_eq!(fee_amount, treasury_fee_diff);

		// Verify omnipool sell
		assert!(user_acc.balance(DAI) > 0);

		let user_hdx_balance = user_acc.balance(HDX);
		assert!(user_hdx_balance < initial_user_hdx_balance);
		let hdx_diff = initial_user_hdx_balance - user_hdx_balance;
		assert_eq!(hdx_diff, sell_amount);
	})
}

#[test]
fn evm_permit_should_validate_unsigned_correctly() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());
	let fee_currency = WETH;

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - bind and fund
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			user_acc.address(),
			fee_currency,
			to_ether(1),
			0,
		));

		//Fund some HDX to sell in omnipool
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			(10 * UNITS) as i128,
		));

		let initial_treasury_fee_balance = treasury_acc.balance(fee_currency);
		let initial_user_hdx_balance = user_acc.balance(HDX);
		let initial_user_fee_currency_balance = user_acc.balance(fee_currency);
		let initial_user_dai_balance = user_acc.balance(DAI);

		// just reset the weth balance to 0 - to make sure we don't have enough DAI
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DAI,
			-(initial_user_dai_balance as i128),
		));
		let initial_user_dai_balance = user_acc.balance(DAI);
		assert_eq!(initial_user_dai_balance, 0);

		//Act
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
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
		let tag: Vec<u8> = ("EVMPermit", (U256::zero(), user_evm_address)).encode();
		assert_eq!(
			MultiTransactionPayment::validate_unsigned(TransactionSource::External, &call),
			Ok(ValidTransaction {
				priority: 0,
				requires: vec![],
				provides: vec![tag],
				longevity: 64,
				propagate: true,
			})
		);

		// Verify that nothing has changed
		assert_eq!(user_acc.balance(fee_currency), initial_user_fee_currency_balance);
		assert_eq!(treasury_acc.balance(fee_currency), initial_treasury_fee_balance);

		// Verify omnipool sell not happened
		assert_eq!(user_acc.balance(DAI), 0);
		assert_eq!(initial_user_hdx_balance, user_acc.balance(HDX));
	})
}

#[test]
fn evm_permit_should_validate_unsigned_correctly_and_return_error_if_inner_call_fails() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());
	let fee_currency = WETH;

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - check there is no fee payment asset to get an error
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			user_acc.address(),
			fee_currency,
			to_ether(1),
			0,
		));

		// Ensure omnipool sell should fail
		assert_eq!(user_acc.balance(HDX), 0);

		let initial_treasury_fee_balance = treasury_acc.balance(fee_currency);
		let initial_user_fee_currency_balance = user_acc.balance(fee_currency);
		let initial_user_dai_balance = user_acc.balance(DAI);
		let initial_user_hdx_balance = user_acc.balance(HDX);

		// just reset the weth balance to 0 - to make sure we don't have enough DAI
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DAI,
			-(initial_user_dai_balance as i128),
		));
		let initial_user_dai_balance = user_acc.balance(DAI);
		assert_eq!(initial_user_dai_balance, 0);

		//Act
		let sell_amount = 10_000_000;
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: sell_amount,
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
		assert_noop!(
			MultiTransactionPayment::validate_unsigned(TransactionSource::External, &call),
			TransactionValidityError::Invalid(InvalidTransaction::Custom(10))
		);

		// Verify that nothing has changed
		assert_eq!(user_acc.balance(fee_currency), initial_user_fee_currency_balance);
		assert_eq!(treasury_acc.balance(fee_currency), initial_treasury_fee_balance);

		// Verify omnipool sell not happened
		assert_eq!(user_acc.balance(DAI), 0);
		assert_eq!(initial_user_hdx_balance, user_acc.balance(HDX));
	})
}

#[test]
fn evm_permit_set_currency_dispatch_should_pay_evm_fee_in_chosen_currency() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let fee_currency = DAI;

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - bind and fund
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			fee_currency,
			100_000_000_000_000_000_000i128,
		));
		let initial_user_fee_currency_balance = user_acc.balance(fee_currency);
		let initial_user_weth_balance = user_acc.balance(WETH);

		let initial_fee_currency_issuance = Currencies::total_issuance(fee_currency);

		// just reset the weth balance to 0 - to make sure we don't have enough WETH
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			WETH,
			-(initial_user_weth_balance as i128),
		));
		let initial_user_weth_balance = user_acc.balance(WETH);
		assert_eq!(initial_user_weth_balance, 0);

		let set_currency_call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: fee_currency },
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

		let tag: Vec<u8> = ("EVMPermit", (U256::zero(), user_evm_address)).encode();
		assert_eq!(
			MultiTransactionPayment::validate_unsigned(TransactionSource::External, &call),
			Ok(ValidTransaction {
				priority: 0,
				requires: vec![],
				provides: vec![tag],
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
		assert_eq!(currency, fee_currency);

		let fee_currency_issuance = Currencies::total_issuance(fee_currency);
		assert_eq!(initial_fee_currency_issuance, fee_currency_issuance);

		let user_fee_currency_balance = user_acc.balance(fee_currency);
		assert!(user_fee_currency_balance < initial_user_fee_currency_balance);

		let fee_diff = initial_user_fee_currency_balance - user_fee_currency_balance;
		assert!(fee_diff > 1000 * UNITS);
	})
}

#[test]
fn evm_permit_set_currency_dispatch_should_pay_evm_fee_in_insufficient_asset() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let initial_fee_currency = WETH;

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			init_omnipool_with_oracle_for_block_10();
			pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
				hydradx_runtime::MinimumMultiplier::get(),
			);
			assert_ok!(hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION));

			let name = b"INSUF1".to_vec();

			let insufficient_asset = AssetRegistry::register_insufficient_asset(
				None,
				Some(name.try_into().unwrap()),
				AssetKind::External,
				Some(1_000),
				None,
				None,
				None,
				None,
			)
			.unwrap();

			assert_ok!(Tokens::set_balance(
				RawOrigin::Root.into(),
				user_acc.address(),
				initial_fee_currency,
				to_ether(1),
				0,
			));

			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				user_acc.address(),
				insufficient_asset,
				100_000_000_000_000_000_000i128,
			));

			assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				FixedU128::from_rational(1, 100000),
			));

			create_xyk_pool(insufficient_asset, 100000000000 * UNITS, DOT, 120000000000 * UNITS);
			assert_ok!(hydradx_runtime::EmaOracle::add_oracle(
				RuntimeOrigin::root(),
				primitives::constants::chain::XYK_SOURCE,
				(DOT, insufficient_asset)
			));
			//Populate oracle
			assert_ok!(Currencies::update_balance(
				RawOrigin::Root.into(),
				BOB.into(),
				insufficient_asset,
				2 * UNITS as i128,
			));

			assert_ok!(XYK::sell(
				RuntimeOrigin::signed(BOB.into()),
				insufficient_asset,
				DOT,
				UNITS,
				0,
				false
			));

			let initial_user_init_fee_balance = user_acc.balance(initial_fee_currency);
			let initial_user_insufficient_balance = user_acc.balance(insufficient_asset);
			let initial_insuff_asset_issuance = Currencies::total_issuance(insufficient_asset);

			let set_currency_call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
				pallet_transaction_multi_payment::Call::set_currency {
					currency: insufficient_asset,
				},
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

			let tag: Vec<u8> = ("EVMPermit", (U256::zero(), user_evm_address)).encode();
			assert_eq!(
				MultiTransactionPayment::validate_unsigned(TransactionSource::External, &call),
				Ok(ValidTransaction {
					priority: 0,
					requires: vec![],
					provides: vec![tag],
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

			let currency = pallet_transaction_multi_payment::Pallet::<hydradx_runtime::Runtime>::account_currency(
				&user_acc.address(),
			);
			assert_eq!(currency, insufficient_asset);

			let insuff_asset_issuance = Currencies::total_issuance(insufficient_asset);
			assert_eq!(initial_insuff_asset_issuance, insuff_asset_issuance);

			let user_insufficient_asset_balance = user_acc.balance(insufficient_asset);
			assert!(user_insufficient_asset_balance < initial_user_insufficient_balance);

			assert_eq!(user_acc.balance(initial_fee_currency), initial_user_init_fee_balance);

			let payed_fee = initial_user_insufficient_balance - user_insufficient_asset_balance;
			assert!(
				payed_fee > 50_000_000,
				"payed_fee: {:?} is less than 50_000_000",
				payed_fee
			);
			assert!(
				payed_fee < 120_000_000,
				"payed_fee: {:?} is more than 120_000_000",
				payed_fee
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	})
}

#[test]
fn convert_amount_should_work_when_converting_insufficient_to_sufficient_asset() {
	TestNet::reset();
	let user_acc = MockAccount::new(alith_truncated_account());
	let initial_fee_currency = WETH;
	let sufficient_currency = HDX;

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			init_omnipool_with_oracle_for_block_10();
			pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
				hydradx_runtime::MinimumMultiplier::get(),
			);
			assert_ok!(hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION));

			let name = b"INSUF1".to_vec();

			let insufficient_asset = AssetRegistry::register_insufficient_asset(
				None,
				Some(name.try_into().unwrap()),
				AssetKind::External,
				Some(1_000),
				None,
				None,
				None,
				None,
			)
			.unwrap();

			assert_ok!(Tokens::set_balance(
				RawOrigin::Root.into(),
				user_acc.address(),
				initial_fee_currency,
				to_ether(1),
				0,
			));

			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				user_acc.address(),
				insufficient_asset,
				100_000_000_000_000_000_000i128,
			));

			assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				FixedU128::from_rational(1, 100000),
			));

			create_xyk_pool(insufficient_asset, 100000000000 * UNITS, DOT, 120000000000 * UNITS);
			assert_ok!(hydradx_runtime::EmaOracle::add_oracle(
				RuntimeOrigin::root(),
				primitives::constants::chain::XYK_SOURCE,
				(DOT, insufficient_asset)
			));
			//Populate oracle
			assert_ok!(Currencies::update_balance(
				RawOrigin::Root.into(),
				BOB.into(),
				insufficient_asset,
				2 * UNITS as i128,
			));
			assert_ok!(XYK::sell(
				RuntimeOrigin::signed(BOB.into()),
				insufficient_asset,
				DOT,
				UNITS,
				0,
				false
			));

			//Convert insufficient to sufficient (WETH)
			type Convert = ConvertBalance<ShortOraclePrice, XykPaymentAssetSupport, DotAssetId>;

			let insufficient_amount = 10 * UNITS;
			let amount_in_sufficient =
				Convert::convert((insufficient_asset, sufficient_currency, insufficient_amount)).unwrap();

			//Assert if we get similar result when selling WETH for insufficient
			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				BOB.into(),
				sufficient_currency,
				amount_in_sufficient.0 as i128,
			));
			let bob_init_dot = Currencies::free_balance(DOT, &AccountId::from(BOB));
			assert_ok!(hydradx_runtime::Omnipool::sell(
				hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
				sufficient_currency,
				DOT,
				amount_in_sufficient.0, //weth needed for the transaction
				0
			));
			let bob_new_dot = Currencies::free_balance(DOT, &AccountId::from(BOB));
			let dot_diff = bob_new_dot - bob_init_dot;

			assert_eq!(user_acc.balance(sufficient_currency), 0);

			let initial_user_insufficient_balance = Currencies::free_balance(insufficient_asset, &AccountId::from(BOB));

			assert_ok!(XYK::sell(
				RuntimeOrigin::signed(BOB.into()),
				DOT,
				insufficient_asset,
				dot_diff,
				0,
				false
			));
			let new_user_insufficient_balance = Currencies::free_balance(insufficient_asset, &AccountId::from(BOB));
			let diff = new_user_insufficient_balance - initial_user_insufficient_balance;

			let difference = insufficient_amount - diff;
			let relative_difference = FixedU128::from_rational(difference, insufficient_amount);
			let tolerated_difference = FixedU128::from_rational(2, 100); //2% due to fees, etc
			assert!(relative_difference < tolerated_difference);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	})
}

#[test]
fn convert_amount_should_fail_gracefully_when_no_xyk_pool_for_fee_payment_asset() {
	TestNet::reset();
	let user_acc = MockAccount::new(alith_truncated_account());

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			init_omnipool_with_oracle_for_block_10();
			pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
				hydradx_runtime::MinimumMultiplier::get(),
			);
			assert_ok!(hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION));

			let name = b"INSUF1".to_vec();

			let insufficient_asset = AssetRegistry::register_insufficient_asset(
				None,
				Some(name.try_into().unwrap()),
				AssetKind::External,
				Some(1_000),
				None,
				None,
				None,
				None,
			)
			.unwrap();

			// Give some WETH to pay fees
			assert_ok!(Tokens::set_balance(
				RawOrigin::Root.into(),
				evm_account(),
				WETH,
				to_ether(1),
				0,
			));

			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				user_acc.address(),
				insufficient_asset,
				100_000_000_000_000_000_000i128,
			));

			assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				FixedU128::from_rational(1, 100000),
			));

			//Populate oracle
			assert_ok!(Currencies::update_balance(
				RawOrigin::Root.into(),
				BOB.into(),
				insufficient_asset,
				2 * UNITS as i128,
			));

			//Convert insufficient to sufficient (WETH) should fail as no corresponding XYK pool
			type Convert = ConvertBalance<ShortOraclePrice, XykPaymentAssetSupport, DotAssetId>;

			let insufficient_amount = 10 * UNITS;
			let amount_in_weth = Convert::convert((insufficient_asset, WETH, insufficient_amount));
			assert!(amount_in_weth.is_none());

			// Assert no balance was acquired; only the fee was paid
			assert!(user_acc.balance(WETH) < to_ether(1));

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	})
}

#[test]
fn convert_amount_should_work_when_converting_sufficient_to_insufficient_asset() {
	TestNet::reset();
	let user_acc = MockAccount::new(alith_truncated_account());
	let fee_currency = WETH;
	let sufficient_currency = HDX;

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			init_omnipool_with_oracle_for_block_10();
			pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
				hydradx_runtime::MinimumMultiplier::get(),
			);
			assert_ok!(hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION));

			let name = b"INSUF1".to_vec();

			let insufficient_asset = AssetRegistry::register_insufficient_asset(
				None,
				Some(name.try_into().unwrap()),
				AssetKind::External,
				Some(1_000),
				None,
				None,
				None,
				None,
			)
			.unwrap();

			// Give some WETH to pay fees
			assert_ok!(Tokens::set_balance(
				RawOrigin::Root.into(),
				user_acc.address(),
				fee_currency,
				to_ether(1),
				0,
			));

			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				user_acc.address(),
				insufficient_asset,
				100_000_000_000_000_000_000i128,
			));

			assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				FixedU128::from_rational(1, 100000),
			));

			create_xyk_pool(insufficient_asset, 100000000000 * UNITS, DOT, 120000000000 * UNITS);
			assert_ok!(hydradx_runtime::EmaOracle::add_oracle(
				RuntimeOrigin::root(),
				primitives::constants::chain::XYK_SOURCE,
				(DOT, insufficient_asset)
			));
			//Populate oracle
			assert_ok!(Currencies::update_balance(
				RawOrigin::Root.into(),
				BOB.into(),
				insufficient_asset,
				2 * UNITS as i128,
			));

			assert_ok!(XYK::sell(
				RuntimeOrigin::signed(BOB.into()),
				insufficient_asset,
				DOT,
				UNITS,
				0,
				false
			));

			//Convert sufficient (HDX) to insufficient
			type Convert = ConvertBalance<ShortOraclePrice, XykPaymentAssetSupport, DotAssetId>;

			let sufficient_amount = 10 * UNITS;
			let amount_in_insufficient_asset =
				Convert::convert((sufficient_currency, insufficient_asset, sufficient_amount)).unwrap();

			let initial_user_dot_balance = Currencies::free_balance(DOT, &AccountId::from(BOB));

			assert_ok!(XYK::sell(
				RuntimeOrigin::signed(BOB.into()),
				insufficient_asset,
				DOT,
				amount_in_insufficient_asset.0,
				0,
				false
			));
			let new_user_dot_balance = Currencies::free_balance(DOT, &AccountId::from(BOB));
			let dot_diff = new_user_dot_balance - initial_user_dot_balance;

			//Assert if we get similar result when selling WETH for insufficient
			let bob_init_sufficient = Currencies::free_balance(sufficient_currency, &AccountId::from(BOB));
			assert_ok!(hydradx_runtime::Omnipool::sell(
				hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
				DOT,
				sufficient_currency,
				dot_diff,
				0
			));
			let bob_new_sufficient = Currencies::free_balance(sufficient_currency, &AccountId::from(BOB));
			let sufficient_diff = bob_new_sufficient - bob_init_sufficient;

			let difference = sufficient_amount - sufficient_diff;
			let relative_difference = FixedU128::from_rational(difference, sufficient_amount);
			let tolerated_difference = FixedU128::from_rational(1, 100); //1% due to fees, etc
			assert!(relative_difference < tolerated_difference);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	})
}

#[test]
fn evm_permit_dispatch_flow_should_work() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());
	let fee_currency = WETH;

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - bind and fund
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			user_acc.address(),
			fee_currency,
			to_ether(1),
			0,
		));

		// Fund some HDX to sell in omnipool
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			(10 * UNITS) as i128,
		));

		let initial_treasury_fee_balance = treasury_acc.balance(fee_currency);
		let initial_user_fee_currency_balance = user_acc.balance(fee_currency);
		let initial_user_dai_balance = user_acc.balance(DAI);
		let initial_user_hdx_balance = user_acc.balance(HDX);

		// just reset the weth balance to 0 - to make sure we don't have enough DAI
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DAI,
			-(initial_user_dai_balance as i128),
		));
		let initial_user_dai_balance = user_acc.balance(DAI);
		assert_eq!(initial_user_dai_balance, 0);

		//Act
		let sell_amount = 10_000_000;
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: sell_amount,
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

		let tag: Vec<u8> = ("EVMPermit", (U256::zero(), user_evm_address)).encode();
		assert_eq!(
			MultiTransactionPayment::validate_unsigned(TransactionSource::External, &call),
			Ok(ValidTransaction {
				priority: 0,
				requires: vec![],
				provides: vec![tag],
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
		let fee_amount = initial_user_fee_currency_balance - user_acc.balance(fee_currency);
		assert!(fee_amount > 0);

		let new_treasury_fee_balance = treasury_acc.balance(fee_currency);
		let treasury_fee_diff = new_treasury_fee_balance - initial_treasury_fee_balance;
		assert_eq!(fee_amount, treasury_fee_diff);

		// Verify omnipool sell
		assert!(user_acc.balance(DAI) > 0);

		let user_hdx_balance = user_acc.balance(HDX);
		assert!(user_hdx_balance < initial_user_hdx_balance);
		let hdx_diff = initial_user_hdx_balance - user_hdx_balance;
		assert_eq!(hdx_diff, sell_amount);
	})
}

#[test]
fn evm_permit_should_fail_when_replayed() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());
	let fee_currency = WETH;

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - bind and fund
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			user_acc.address(),
			fee_currency,
			to_ether(1),
			0,
		));

		// Fund some HDX to sell in omnipool
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			(10 * UNITS) as i128,
		));

		let initial_treasury_fee_balance = treasury_acc.balance(fee_currency);
		let initial_user_fee_currency_balance = user_acc.balance(fee_currency);
		let initial_user_dai_balance = user_acc.balance(DAI);
		let initial_user_hdx_balance = user_acc.balance(HDX);

		// just reset the weth balance to 0 - to make sure we don't have enough DAI
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DAI,
			-(initial_user_dai_balance as i128),
		));
		let initial_user_dai_balance = user_acc.balance(DAI);
		assert_eq!(initial_user_dai_balance, 0);

		//Act
		let sell_amount = 10_000_000;
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: sell_amount,
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

		let tag: Vec<u8> = ("EVMPermit", (U256::zero(), user_evm_address)).encode();
		assert_eq!(
			MultiTransactionPayment::validate_unsigned(TransactionSource::External, &call),
			Ok(ValidTransaction {
				priority: 0,
				requires: vec![],
				provides: vec![tag],
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
			MultiTransactionPayment::validate_unsigned(TransactionSource::External, &call),
			TransactionValidityError::Invalid(InvalidTransaction::Custom(9))
		);

		// Verify evm fee amount
		let fee_amount = initial_user_fee_currency_balance - user_acc.balance(fee_currency);
		assert!(fee_amount > 0);

		let treasury_fee_diff = treasury_acc.balance(fee_currency) - initial_treasury_fee_balance;
		assert_eq!(fee_amount, treasury_fee_diff);

		// Verify omnipool sell
		assert!(user_acc.balance(DAI) > 0);

		let user_hdx_balance = user_acc.balance(HDX);
		assert!(user_hdx_balance < initial_user_hdx_balance);
		let hdx_diff = initial_user_hdx_balance - user_hdx_balance;
		assert_eq!(hdx_diff, 10_000_000);
	})
}

#[test]
fn dispatch_permit_should_increase_account_nonce_correctly() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let fee_currency = WETH;

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();

		let evm_account_nonces = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonces, U256::zero());
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - bind and fund
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			user_acc.address(),
			fee_currency,
			to_ether(1),
			0,
		));

		// Fund some HDX to sell in omnipool
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			(10 * UNITS) as i128,
		));

		let initial_user_fee_balance = user_acc.balance(fee_currency);
		let initial_user_dai_balance = user_acc.balance(DAI);

		// just reset the weth balance to 0 - to make sure we don't have enough DAI
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DAI,
			-(initial_user_dai_balance as i128),
		));
		let initial_user_dai_balance = user_acc.balance(DAI);
		assert_eq!(initial_user_dai_balance, 0);

		//Act
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
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

		let tag: Vec<u8> = ("EVMPermit", (U256::zero(), user_evm_address)).encode();
		assert_eq!(
			MultiTransactionPayment::validate_unsigned(TransactionSource::External, &call),
			Ok(ValidTransaction {
				priority: 0,
				requires: vec![],
				provides: vec![tag],
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

		// Verify account nonces
		let evm_account_nonce = user_acc.nonce();
		assert_eq!(evm_account_nonce, Index::zero());

		let permit_nonce =
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				user_evm_address,
			);
		assert_eq!(permit_nonce, U256::one());

		let tx_fee = initial_user_fee_balance - user_acc.balance(fee_currency);
		assert!(tx_fee > 0);
	})
}

#[test]
fn dispatch_permit_should_increase_permit_nonce_when_call_fails() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let fee_currency = WETH;

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();

		let evm_account_nonces = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonces, U256::zero());
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - bind and fund
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			user_acc.address(),
			fee_currency,
			to_ether(1),
			0,
		));

		let initial_user_dai_balance = user_acc.balance(DAI);
		let initial_user_hdx_balance = user_acc.balance(HDX);
		assert_eq!(initial_user_hdx_balance, 0);

		// just reset the weth balance to 0 - to make sure we don't have enough DAI
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DAI,
			-(initial_user_dai_balance as i128),
		));
		let initial_user_dai_balance = user_acc.balance(DAI);
		assert_eq!(initial_user_dai_balance, 0);

		//Act
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
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

		assert_eq!(user_acc.balance(DAI), 0);

		// Verify account nonces
		let evm_account_nonce = user_acc.nonce();
		assert_eq!(evm_account_nonce, Index::zero());

		let permit_nonce =
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				user_evm_address,
			);
		assert_eq!(permit_nonce, U256::one());
	})
}

#[test]
fn dispatch_permit_should_charge_tx_fee_when_call_fails() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let fee_currency = WETH;

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		let evm_account_nonces = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonces, U256::zero());
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - bind and fund
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			user_acc.address(),
			fee_currency,
			to_ether(1),
			0,
		));

		let initial_user_fee_currency_balance = user_acc.balance(fee_currency);
		let initial_user_dai_balance = user_acc.balance(DAI);
		let initial_user_hdx_balance = user_acc.balance(HDX);
		assert_eq!(initial_user_hdx_balance, 0);

		// just reset the weth balance to 0 - to make sure we don't have enough DAI
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DAI,
			-(initial_user_dai_balance as i128),
		));
		let initial_user_dai_balance = user_acc.balance(DAI);
		assert_eq!(initial_user_dai_balance, 0);

		//Act
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
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

		assert_eq!(user_acc.balance(DAI), 0);

		// Verify account nonces
		let evm_account_nonce = user_acc.nonce();
		assert_eq!(evm_account_nonce, Index::zero());

		let permit_nonce =
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				user_evm_address,
			);
		assert_eq!(permit_nonce, U256::one());

		let tx_fee = initial_user_fee_currency_balance - user_acc.balance(fee_currency);

		assert_ne!(tx_fee, 0);
	})
}

#[test]
fn dispatch_permit_should_pause_tx_when_permit_is_invalid() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		let evm_account_nonce = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonce, U256::zero());
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
		let initial_user_hdx_balance = user_acc.balance(HDX);
		let initial_user_weth_balance = user_acc.balance(WETH);
		let initial_user_dot_balance = user_acc.balance(DOT);
		assert_eq!(initial_user_dot_balance, 0);

		// just reset the weth balance to 0 - to make sure we don't have enough WETH
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

		assert_ok!(MultiTransactionPayment::dispatch_permit(
			hydradx_runtime::RuntimeOrigin::none(),
			user_evm_address,
			DISPATCH_ADDR,
			U256::from(1),
			omni_sell.encode(),
			gas_limit,
			deadline,
			v.serialize(),
			H256::from(rs.r.b32()),
			H256::from(rs.s.b32()),
		));

		assert_eq!(user_acc.balance(WETH), 0);

		// Verify account nonces
		let evm_account_nonce = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonce, U256::zero());

		let permit_nonce =
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				user_evm_address,
			);
		assert_eq!(permit_nonce, U256::zero());

		let hdx_balance = user_acc.balance(HDX);
		let tx_fee = initial_user_hdx_balance - hdx_balance;

		assert_eq!(tx_fee, 0);

		let call = RuntimeCall::MultiTransactionPayment(pallet_transaction_multi_payment::Call::dispatch_permit {
			from: user_evm_address,
			to: DISPATCH_ADDR,
			value: U256::from(0),
			data: omni_sell.encode(),
			gas_limit,
			deadline,
			v: v.serialize(),
			r: H256::from(rs.r.b32()),
			s: H256::from(rs.s.b32()),
		});
		assert!(pallet_transaction_pause::PausedTransactionFilter::<
			hydradx_runtime::Runtime,
		>::contains(&call));
	})
}

#[test]
fn dispatch_permit_should_not_pause_tx_when_call_execution_fails() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let fee_currency = WETH;

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		let evm_account_nonces = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonces, U256::zero());
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		// Prepare user evm account - bind and fund
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			user_acc.address(),
			fee_currency,
			to_ether(1),
			0
		));

		let initial_user_fee_currency_balance = user_acc.balance(fee_currency);
		let initial_user_dai_balance = user_acc.balance(DAI);
		let initial_user_hdx_balance = user_acc.balance(HDX);
		assert_eq!(initial_user_hdx_balance, 0);

		// just reset the weth balance to 0 - to make sure we don't have enough DAI
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DAI,
			-(initial_user_dai_balance as i128),
		));
		let initial_user_dai_balance = user_acc.balance(DAI);
		assert_eq!(initial_user_dai_balance, 0);

		//Act
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
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

		assert_eq!(user_acc.balance(DAI), 0);

		// Verify account nonces
		let evm_account_nonce = user_acc.nonce();
		assert_eq!(evm_account_nonce, Index::zero());

		let permit_nonce =
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				user_evm_address,
			);
		assert_eq!(permit_nonce, U256::one());

		let tx_fee = initial_user_fee_currency_balance - user_acc.balance(fee_currency);
		assert!(tx_fee > 0);

		let call = RuntimeCall::MultiTransactionPayment(pallet_transaction_multi_payment::Call::dispatch_permit {
			from: user_evm_address,
			to: DISPATCH_ADDR,
			value: U256::from(0),
			data: omni_sell.encode(),
			gas_limit,
			deadline,
			v: v.serialize(),
			r: H256::from(rs.r.b32()),
			s: H256::from(rs.s.b32()),
		});
		assert!(!pallet_transaction_pause::PausedTransactionFilter::<
			hydradx_runtime::Runtime,
		>::contains(&call));
	})
}

#[test]
fn dispatch_permit_should_pause_tx_when_no_tx_fee_is_paid() {
	TestNet::reset();
	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		let evm_account_nonce = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonce, U256::zero());
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		let initial_user_hdx_balance = user_acc.balance(HDX);
		let initial_user_weth_balance = user_acc.balance(WETH);
		let initial_user_dot_balance = user_acc.balance(DOT);
		assert_eq!(initial_user_dot_balance, 0);

		// just reset the weth balance to 0 - to make sure we don't have enough WETH to pay fees
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

		assert_eq!(user_acc.balance(WETH), 0);

		// Verify account nonces
		let evm_account_nonce = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonce, U256::zero());

		let permit_nonce =
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				user_evm_address,
			);
		assert_eq!(permit_nonce, U256::zero());

		let hdx_balance = user_acc.balance(HDX);
		let tx_fee = initial_user_hdx_balance - hdx_balance;

		assert_eq!(tx_fee, 0);

		let call = RuntimeCall::MultiTransactionPayment(pallet_transaction_multi_payment::Call::dispatch_permit {
			from: user_evm_address,
			to: DISPATCH_ADDR,
			value: U256::from(0),
			data: omni_sell.encode(),
			gas_limit,
			deadline,
			v: v.serialize(),
			r: H256::from(rs.r.b32()),
			s: H256::from(rs.s.b32()),
		});
		assert!(pallet_transaction_pause::PausedTransactionFilter::<
			hydradx_runtime::Runtime,
		>::contains(&call));
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
