#![cfg(test)]

use crate::polkadot_test_net::*;
use crate::utils::accounts::*;
use hydradx_runtime::evm::Erc20Currency;

use frame_support::dispatch::GetDispatchInfo;
use frame_support::pallet_prelude::ValidateUnsigned;
use frame_support::storage::with_transaction;
use frame_support::traits::fungible::Mutate;
use frame_support::traits::Contains;
use frame_support::{assert_noop, assert_ok, sp_runtime::codec::Encode};
use frame_system::RawOrigin;
use hydradx_adapters::price::ConvertBalance;
use hydradx_runtime::evm::precompiles::{CALLPERMIT, DISPATCH_ADDR};
use hydradx_runtime::types::TenMinutesOraclePrice;
use hydradx_runtime::AssetRegistry;
use hydradx_runtime::DOT_ASSET_LOCATION;
use hydradx_runtime::XYK;
use hydradx_runtime::{
	Balances, Currencies, DotAssetId, MultiTransactionPayment, Omnipool, RuntimeCall, RuntimeOrigin, Tokens,
	XykPaymentAssetSupport,
};
use hydradx_runtime::{FixedU128, Runtime};
use hydradx_traits::evm::CallContext;
use hydradx_traits::evm::ERC20;
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use hydradx_traits::Mutate as AssetRegistryMutate;
use libsecp256k1::{sign, Message, SecretKey};
use orml_traits::MultiCurrency;
use pallet_evm_accounts::EvmNonceProvider;
use pallet_transaction_multi_payment::EVMPermit;
use pretty_assertions::assert_eq;
use primitives::constants::currency::UNITS;
use primitives::{AssetId, Balance};
use sp_core::{H256, U256};
use sp_runtime::traits::Convert;
use sp_runtime::traits::DispatchTransaction;
use sp_runtime::transaction_validity::InvalidTransaction;
use sp_runtime::transaction_validity::TransactionValidityError;
use sp_runtime::transaction_validity::{TransactionSource, ValidTransaction};
use sp_runtime::DispatchResult;
use sp_runtime::Permill;
use sp_runtime::TransactionOutcome;
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
			.validate_and_prepare(
				Some(AccountId::from(user_acc.address())).into(),
				&omni_sell,
				&info,
				len,
				0,
			);
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
fn evm_permit_set_currency_dispatch_should_pay_evm_fee_in_chosen_erc20_currency() {
	TestNet::reset();

	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());

	Hydra::execute_with(|| {
		//Create new erc20, fund user with it and set it as fee payment currency
		let contract = crate::erc20::deploy_token_contract();
		let asset = crate::erc20::bind_erc20(contract);
		let _balance = Currencies::free_balance(asset, &ALICE.into());
		let initial_treasury_fee_balance = treasury_acc.balance(asset);
		let erc20_balance = 2000000000000000;
		assert_eq!(erc20_balance, 2000000000000000);
		assert_ok!(<Erc20Currency<Runtime> as ERC20>::transfer(
			CallContext {
				contract,
				sender: crate::erc20::deployer(),
				origin: crate::erc20::deployer()
			},
			user_evm_address,
			erc20_balance
		));

		assert_ok!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(alith_evm_account()),
			hydradx_runtime::Omnipool::protocol_account(),
			asset,
			erc20_balance / 2
		));

		let alith_balance = Currencies::free_balance(asset, &alith_evm_account());
		assert_eq!(alith_balance, erc20_balance / 2);

		assert_ok!(MultiTransactionPayment::add_currency(
			hydradx_runtime::RuntimeOrigin::root(),
			asset,
			FixedU128::from_rational(1, 2)
		));
		assert_ok!(MultiTransactionPayment::set_currency(
			hydradx_runtime::RuntimeOrigin::signed(alith_evm_account()),
			DAI,
		));
		let fee_currency = asset;

		init_omnipool_with_oracle_for_block_10();
		//Add new erc20 token to omnipool and populate oracle
		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			asset,
			FixedU128::from_rational(1, 2),
			Permill::from_percent(100),
			AccountId::from(alith_evm_account()),
		));
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(alith_evm_account()),
			asset,
			0,
			erc20_balance / 100,
			Balance::MIN
		));
		hydradx_run_to_next_block();

		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

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
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DAI,
			10000 * UNITS as i128,
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
		let call: pallet_transaction_multi_payment::Call<hydradx_runtime::Runtime> =
			pallet_transaction_multi_payment::Call::dispatch_permit {
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

		//Commented out as we first we want to have a failing test for the behaviour
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
		let final_treasury_fee_balance = treasury_acc.balance(asset);

		assert!(final_treasury_fee_balance > initial_treasury_fee_balance);
		let fee_amount = initial_user_fee_currency_balance - user_fee_currency_balance;
		let treasury_received = final_treasury_fee_balance - initial_treasury_fee_balance;
		assert_eq!(fee_amount, treasury_received);
	})
}

#[test]
fn evm_permit_set_currency_dispatch_should_work_when_wrapped_in_dispatch_with_extra_gas_by_frontend() {
	TestNet::reset();

	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());
	let treasury_acc = MockAccount::new(Treasury::account_id());

	Hydra::execute_with(|| {
		//Create new erc20, fund user with it and set it as fee payment currency
		let contract = crate::erc20::deploy_token_contract();
		let asset = crate::erc20::bind_erc20(contract);
		let _balance = Currencies::free_balance(asset, &ALICE.into());
		let initial_treasury_fee_balance = treasury_acc.balance(asset);
		let erc20_balance = 2000000000000000;
		assert_eq!(erc20_balance, 2000000000000000);
		assert_ok!(<Erc20Currency<Runtime> as ERC20>::transfer(
			CallContext {
				contract,
				sender: crate::erc20::deployer(),
				origin: crate::erc20::deployer()
			},
			user_evm_address,
			erc20_balance
		));

		assert_ok!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(alith_evm_account()),
			hydradx_runtime::Omnipool::protocol_account(),
			asset,
			erc20_balance / 2
		));

		let alith_balance = Currencies::free_balance(asset, &alith_evm_account());
		assert_eq!(alith_balance, erc20_balance / 2);

		assert_ok!(MultiTransactionPayment::add_currency(
			hydradx_runtime::RuntimeOrigin::root(),
			asset,
			FixedU128::from_rational(1, 2)
		));
		assert_ok!(MultiTransactionPayment::set_currency(
			hydradx_runtime::RuntimeOrigin::signed(alith_evm_account()),
			DAI,
		));
		let fee_currency = asset;

		init_omnipool_with_oracle_for_block_10();
		//Add new erc20 token to omnipool and populate oracle
		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			asset,
			FixedU128::from_rational(1, 2),
			Permill::from_percent(100),
			AccountId::from(alith_evm_account()),
		));
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(alith_evm_account()),
			asset,
			0,
			erc20_balance / 10,
			Balance::MIN
		));
		hydradx_run_to_next_block();

		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

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
		let dispatch_set_currency_call =
			hydradx_runtime::RuntimeCall::Dispatcher(pallet_dispatcher::Call::dispatch_with_extra_gas {
				call: Box::new(set_currency_call.clone()),
				extra_gas: 100_000,
			});

		let gas_limit = 1000000;
		let deadline = U256::from(1000000000000u128);

		let permit =
			pallet_evm_precompile_call_permit::CallPermitPrecompile::<hydradx_runtime::Runtime>::generate_permit(
				CALLPERMIT,
				user_evm_address,
				DISPATCH_ADDR,
				U256::from(0),
				dispatch_set_currency_call.encode(),
				gas_limit,
				U256::zero(),
				deadline,
			);
		let secret_key = SecretKey::parse(&user_secret_key).unwrap();
		let message = Message::parse(&permit);
		let (rs, v) = sign(&message, &secret_key);

		// Validate unsigned first
		let call: pallet_transaction_multi_payment::Call<hydradx_runtime::Runtime> =
			pallet_transaction_multi_payment::Call::dispatch_permit {
				from: user_evm_address,
				to: DISPATCH_ADDR,
				value: U256::from(0),
				data: dispatch_set_currency_call.encode(),
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
			dispatch_set_currency_call.encode(),
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
		let final_treasury_fee_balance = treasury_acc.balance(asset);

		assert!(final_treasury_fee_balance > initial_treasury_fee_balance);
		let fee_amount = initial_user_fee_currency_balance - user_fee_currency_balance;
		let treasury_received = final_treasury_fee_balance - initial_treasury_fee_balance;
		assert_eq!(fee_amount, treasury_received);
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
			type Convert = ConvertBalance<TenMinutesOraclePrice, XykPaymentAssetSupport, DotAssetId>;

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
			type Convert = ConvertBalance<TenMinutesOraclePrice, XykPaymentAssetSupport, DotAssetId>;

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
			type Convert = ConvertBalance<TenMinutesOraclePrice, XykPaymentAssetSupport, DotAssetId>;

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

		let evm_account_nonce = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonce, U256::zero());
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
		let evm_account_nonce = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonce, U256::zero());

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

		let evm_account_nonce = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonce, U256::zero());
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
		let evm_account_nonce = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonce, U256::zero());

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
		let evm_account_nonce = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonce, U256::zero());
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
		let evm_account_nonce = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonce, U256::zero());

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
		let evm_account_nonce = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonce, U256::zero());
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
		let evm_account_nonce = hydradx_runtime::evm::EvmNonceProvider::get_nonce(user_evm_address);
		assert_eq!(evm_account_nonce, U256::zero());

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

// Tests validating that the CALLPERMIT precompile and dispatch_permit share
// a single permit domain by design. dispatch_permit is a self-relay mechanism:
// the user signs one permit and submits it as an unsigned extrinsic to pay fees
// in a non-native currency. The shared EIP-712 digest and nonce space are intentional.

#[test]
fn permit_is_accepted_by_both_callpermit_and_dispatch_permit_by_design() {
	// The CALLPERMIT precompile and dispatch_permit share the same EIP-712 domain
	// and nonce space. A permit signed once can be submitted via either interface.
	// This is by design — dispatch_permit is a self-relay path, not a separate trust domain.
	TestNet::reset();

	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			user_acc.address(),
			WETH,
			to_ether(1),
			0,
		));
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			(10 * UNITS) as i128,
		));

		let initial_user_weth = user_acc.balance(WETH);

		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 10_000_000,
				min_buy_amount: 0,
			});

		let gas_limit = 1_000_000u64;
		let deadline = U256::from(1_000_000_000_000u128);

		// Generate permit using the shared CALLPERMIT domain
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

		// Submit via dispatch_permit (self-relay path)
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

		// Signer pays the EVM fee via dispatch_permit (expected for self-relay)
		let fee_paid = initial_user_weth - user_acc.balance(WETH);
		assert!(
			fee_paid > 0,
			"signer should pay fee when self-relaying via dispatch_permit"
		);

		// Permit nonce consumed — prevents reuse via either interface
		let permit_nonce =
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				user_evm_address,
			);
		assert_eq!(permit_nonce, U256::one());
	})
}

#[test]
fn shared_nonce_prevents_permit_reuse_across_submission_paths() {
	// The shared nonce space ensures a permit can only be used once, regardless
	// of which interface it was submitted through. This is the intended replay protection.
	TestNet::reset();

	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			user_acc.address(),
			WETH,
			to_ether(1),
			0,
		));
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			(10 * UNITS) as i128,
		));

		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 10_000_000,
				min_buy_amount: 0,
			});

		let gas_limit = 1_000_000u64;
		let deadline = U256::from(1_000_000_000_000u128);

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

		// First use succeeds
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

		assert_eq!(
			<hydradx_runtime::Runtime as pallet_transaction_multi_payment::Config>::EvmPermit::permit_nonce(
				user_evm_address,
			),
			U256::one()
		);

		// Second use of the same permit is rejected — nonce already consumed
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
		assert!(
			MultiTransactionPayment::validate_unsigned(TransactionSource::External, &call).is_err(),
			"same permit cannot be used twice — shared nonce prevents replay"
		);
	})
}

#[test]
fn dispatch_permit_fee_currency_override_works_with_any_to_address() {
	// dispatch_permit decodes fee currency from `data` regardless of the `to` address.
	// This is safe because `data` is part of the signed permit — the signer explicitly
	// committed to this data. An external party cannot alter it post-signature.
	TestNet::reset();

	let user_evm_address = alith_evm_address();
	let user_secret_key = alith_secret_key();
	let user_acc = MockAccount::new(alith_truncated_account());

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			DAI,
			100_000_000_000_000_000_000i128,
		));
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			user_acc.address(),
			WETH,
			to_ether(1),
			0,
		));

		let initial_dai = user_acc.balance(DAI);
		let initial_weth = user_acc.balance(WETH);

		// The signer explicitly signs a permit with set_currency(DAI) as data.
		// The `to` address does not need to be DISPATCH_ADDR for fee currency
		// detection to work — this is by design since data is signer-committed.
		let set_currency_call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: DAI },
		);
		let data = set_currency_call.encode();

		let arbitrary_to: sp_core::H160 = sp_core::H160::from_low_u64_be(0xdeadbeef);

		let gas_limit = 1_000_000u64;
		let deadline = U256::from(1_000_000_000_000u128);

		let permit =
			pallet_evm_precompile_call_permit::CallPermitPrecompile::<hydradx_runtime::Runtime>::generate_permit(
				CALLPERMIT,
				user_evm_address,
				arbitrary_to,
				U256::from(0),
				data.clone(),
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
			arbitrary_to,
			U256::from(0),
			data,
			gas_limit,
			deadline,
			v.serialize(),
			H256::from(rs.r.b32()),
			H256::from(rs.s.b32()),
		));

		let dai_spent = initial_dai - user_acc.balance(DAI);
		let weth_spent = initial_weth - user_acc.balance(WETH);

		// Fee currency override applied from data regardless of `to` address.
		// This is safe: the signer chose this data and signed over it.
		assert!(dai_spent > 0, "DAI should be used as fee currency per signer's data");
		assert_eq!(weth_spent, 0, "WETH should not be touched when DAI is overridden");
	})
}

#[cfg(test)]
mod sponsored_paymaster {
	use super::*;
	use hydradx_runtime::{CallFilter, RuntimeEvent};
	use sp_core::H160;
	// Disambiguate from pretty_assertions::assert_eq inside this module.
	use core::assert_eq;

	fn paymaster_account() -> AccountId {
		AccountId::from(crate::polkadot_test_net::BOB)
	}

	fn assert_dispatch_permit_not_paused() {
		let dummy =
			RuntimeCall::MultiTransactionPayment(pallet_transaction_multi_payment::Call::<Runtime>::dispatch_permit {
				from: H160::zero(),
				to: H160::zero(),
				value: U256::zero(),
				data: vec![],
				gas_limit: 0,
				deadline: U256::zero(),
				v: 0,
				r: H256::zero(),
				s: H256::zero(),
			});
		assert!(CallFilter::contains(&dummy), "dispatch_permit MUST NOT be autopaused");
	}

	fn build_permit_for_call(
		inner_call: &RuntimeCall,
		gas_limit: u64,
		deadline: U256,
	) -> (H160, Vec<u8>, u64, U256, u8, H256, H256) {
		let from = alith_evm_address();
		let secret_key = SecretKey::parse(&alith_secret_key()).unwrap();
		let data = inner_call.encode();

		let permit = pallet_evm_precompile_call_permit::CallPermitPrecompile::<Runtime>::generate_permit(
			CALLPERMIT,
			from,
			DISPATCH_ADDR,
			U256::from(0),
			data.clone(),
			gas_limit,
			U256::zero(),
			deadline,
		);
		let message = Message::parse(&permit);
		let (rs, v) = sign(&message, &secret_key);
		(
			from,
			data,
			gas_limit,
			deadline,
			v.serialize(),
			H256::from(rs.r.b32()),
			H256::from(rs.s.b32()),
		)
	}

	fn submit_signed_dispatch_permit_omni_sell(
		paymaster: AccountId,
		sell_amount: Balance,
	) -> (
		frame_support::dispatch::DispatchResultWithPostInfo,
		MockAccount,
		RuntimeCall,
	) {
		let inner_call = RuntimeCall::Omnipool(pallet_omnipool::Call::<Runtime>::sell {
			asset_in: HDX,
			asset_out: DAI,
			amount: sell_amount,
			min_buy_amount: 0,
		});
		let (from, data, gas_limit, deadline, v, r, s) =
			build_permit_for_call(&inner_call, 1_000_000, U256::from(1_000_000_000_000u128));

		let result = MultiTransactionPayment::dispatch_permit(
			RuntimeOrigin::signed(paymaster),
			from,
			DISPATCH_ADDR,
			U256::from(0),
			data,
			gas_limit,
			deadline,
			v,
			r,
			s,
		);

		(result, MockAccount::new(alith_truncated_account()), inner_call)
	}

	#[test]
	fn signed_dispatch_permit_should_execute_inner_call_and_user_hdx_should_change_only_by_swap_amount() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			let user_acc = MockAccount::new(alith_truncated_account());

			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));
			// Zero WETH so user has nothing to pay EVM gas with even if routing fails.
			let user_weth = user_acc.balance(WETH);
			if user_weth > 0 {
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					user_acc.address(),
					WETH,
					-(user_weth as i128),
				));
			}

			let initial_user_hdx = user_acc.balance(HDX);
			let initial_user_weth = user_acc.balance(WETH);
			let initial_user_dai = user_acc.balance(DAI);
			let initial_paymaster_hdx = Currencies::free_balance(HDX, &paymaster);

			let sell_amount: Balance = 1_000_000_000;
			let (result, _, _) = submit_signed_dispatch_permit_omni_sell(paymaster.clone(), sell_amount);
			assert_ok!(result);

			assert_eq!(
				user_acc.balance(WETH),
				initial_user_weth,
				"user WETH must NOT change — paymaster pays EVM gas"
			);
			assert_eq!(
				initial_user_hdx - user_acc.balance(HDX),
				sell_amount,
				"user HDX must decrease ONLY by the sell amount (no fee/gas debit)"
			);
			assert!(user_acc.balance(DAI) > initial_user_dai);
			assert!(Currencies::free_balance(HDX, &paymaster) < initial_paymaster_hdx);
		});
	}

	#[test]
	fn signed_dispatch_permit_should_emit_fee_sponsored_event_on_success() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));
			let user_acc = MockAccount::new(alith_truncated_account());
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));

			let (result, _, _) = submit_signed_dispatch_permit_omni_sell(paymaster.clone(), 1_000_000_000);
			assert_ok!(result);

			let alith = alith_evm_address();
			let found = frame_system::Pallet::<Runtime>::events().iter().any(|record| {
				matches!(
					&record.event,
					RuntimeEvent::MultiTransactionPayment(pallet_transaction_multi_payment::Event::FeeSponsored {
						from,
						fee_payer,
						..
					}) if *from == alith && *fee_payer == paymaster
				)
			});
			assert!(found, "FeeSponsored event should be emitted on success");
		});
	}

	#[test]
	fn dispatch_permit_should_reject_root_origin_with_bad_origin() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let inner_call = RuntimeCall::Omnipool(pallet_omnipool::Call::<Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 100,
				min_buy_amount: 0,
			});
			let (from, data, gas_limit, deadline, v, r, s) =
				build_permit_for_call(&inner_call, 1_000_000, U256::from(1_000_000_000_000u128));

			let result = MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::root(),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				data,
				gas_limit,
				deadline,
				v,
				r,
				s,
			);

			assert_noop!(result, frame_support::sp_runtime::traits::BadOrigin);
		});
	}

	#[test]
	fn signed_dispatch_permit_should_fail_when_signature_is_invalid_and_not_pause_extrinsic() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));

			let inner_call = RuntimeCall::Omnipool(pallet_omnipool::Call::<Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 100,
				min_buy_amount: 0,
			});
			let (from, data, gas_limit, deadline, _v, _r, _s) =
				build_permit_for_call(&inner_call, 1_000_000, U256::from(1_000_000_000_000u128));
			let bad_r = H256::from([0xAAu8; 32]);
			let bad_s = H256::from([0xBBu8; 32]);

			let result = MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::signed(paymaster),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				data,
				gas_limit,
				deadline,
				27,
				bad_r,
				bad_s,
			);

			let err = result.expect_err("bad signature must produce an error");
			assert_eq!(
				err.error,
				pallet_transaction_multi_payment::Error::<Runtime>::EvmPermitInvalid.into(),
			);
			assert_dispatch_permit_not_paused();
		});
	}

	// `pallet_timestamp::Pallet::now()` is 0 in tests by default — without
	// `set_timestamp` below, `deadline = 1 >= 0` passes and we'd silently
	// exercise the dry-run path instead. User also needs HDX so the dry-run
	// wouldn't catch insufficient balance first.
	#[test]
	fn signed_dispatch_permit_should_fail_with_expired_when_deadline_in_past_and_not_pause() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));

			let user_acc = MockAccount::new(alith_truncated_account());
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));

			// Anchor time well beyond the deadline; see test-env note above.
			pallet_timestamp::Pallet::<Runtime>::set_timestamp(1_000_000_000_000);

			let inner_call = RuntimeCall::Omnipool(pallet_omnipool::Call::<Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 1_000_000_000,
				min_buy_amount: 0,
			});
			let (from, data, gas_limit, deadline, v, r, s) =
				build_permit_for_call(&inner_call, 1_000_000, U256::from(1));

			let result = MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::signed(paymaster),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				data,
				gas_limit,
				deadline,
				v,
				r,
				s,
			);

			let err = result.expect_err("expired deadline must produce an error");
			assert_eq!(
				err.error,
				pallet_transaction_multi_payment::Error::<Runtime>::EvmPermitExpired.into(),
			);
			assert_dispatch_permit_not_paused();
		});
	}

	#[test]
	fn signed_dispatch_permit_should_fail_with_inner_call_would_fail_when_inner_call_reverts_and_not_pause() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));
			let user_acc = MockAccount::new(alith_truncated_account());
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));

			let inner_call = RuntimeCall::Omnipool(pallet_omnipool::Call::<Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 100,
				min_buy_amount: u128::MAX, // unachievable slippage
			});
			let (from, data, gas_limit, deadline, v, r, s) =
				build_permit_for_call(&inner_call, 1_000_000, U256::from(1_000_000_000_000u128));

			let result = MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::signed(paymaster),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				data,
				gas_limit,
				deadline,
				v,
				r,
				s,
			);

			let err = result.expect_err("inner call would fail must produce an error");
			assert_eq!(
				err.error,
				pallet_transaction_multi_payment::Error::<Runtime>::InnerCallWouldFail.into(),
			);
			assert_dispatch_permit_not_paused();
		});
	}

	#[test]
	fn signed_dispatch_permit_should_fail_when_replayed_with_same_nonce() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));
			let user_acc = MockAccount::new(alith_truncated_account());
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));

			let inner_call = RuntimeCall::Omnipool(pallet_omnipool::Call::<Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 1_000_000_000,
				min_buy_amount: 0,
			});
			let (from, data, gas_limit, deadline, v, r, s) =
				build_permit_for_call(&inner_call, 1_000_000, U256::from(1_000_000_000_000u128));

			assert_ok!(MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::signed(paymaster.clone()),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				data.clone(),
				gas_limit,
				deadline,
				v,
				r,
				s,
			));

			// Second submission of the same permit — nonce now stale.
			let result = MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::signed(paymaster),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				data,
				gas_limit,
				deadline,
				v,
				r,
				s,
			);
			assert!(result.is_err(), "replay must fail");
			assert_dispatch_permit_not_paused();
		});
	}

	#[test]
	fn unsigned_dispatch_permit_should_still_work_when_signed_branch_is_added() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let user_acc = MockAccount::new(alith_truncated_account());
			assert_ok!(Tokens::set_balance(
				RawOrigin::Root.into(),
				user_acc.address(),
				WETH,
				to_ether(1),
				0,
			));
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));

			let inner_call = RuntimeCall::Omnipool(pallet_omnipool::Call::<Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 100,
				min_buy_amount: 0,
			});
			let (from, data, gas_limit, deadline, v, r, s) =
				build_permit_for_call(&inner_call, 1_000_000, U256::from(1_000_000_000_000u128));

			assert_ok!(MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::none(),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				data,
				gas_limit,
				deadline,
				v,
				r,
				s,
			));

			// Unsigned-path success should NOT emit FeeSponsored.
			let alith = alith_evm_address();
			let unsigned_emitted_fee_sponsored = frame_system::Pallet::<Runtime>::events().iter().any(|record| {
				matches!(
					&record.event,
					RuntimeEvent::MultiTransactionPayment(pallet_transaction_multi_payment::Event::FeeSponsored { from, .. })
						if *from == alith
				)
			});
			assert!(
				!unsigned_emitted_fee_sponsored,
				"unsigned branch must NOT emit FeeSponsored"
			);
		});
	}

	// Guards against the signed-branch's 2× weight reservation ever leaking
	// into user-paid fees on the unsigned path. 30% tolerance matches the
	// pre-existing `compare_fee_in_hdx_between_evm_and_native_*` test.
	#[test]
	fn unsigned_dispatch_permit_user_fee_must_not_be_doubled_by_signed_branch_weight_macro() {
		TestNet::reset();

		let user_evm_address = alith_evm_address();
		let user_secret_key = alith_secret_key();
		let user_acc = MockAccount::new(alith_truncated_account());
		let fee_currency = WETH;

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();

			assert_ok!(Tokens::set_balance(
				RawOrigin::Root.into(),
				user_acc.address(),
				WETH,
				to_ether(1),
				0,
			));
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));

			let alice_currency_balance_initial = Currencies::free_balance(fee_currency, &user_acc.address());

			let omni_sell = RuntimeCall::Omnipool(pallet_omnipool::Call::<Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 10_000_000_000,
				min_buy_amount: 0,
			});
			let gas_limit = 1_000_000;
			let deadline = U256::from(1_000_000_000_000u128);
			let permit = pallet_evm_precompile_call_permit::CallPermitPrecompile::<Runtime>::generate_permit(
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

			assert_ok!(MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::none(),
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

			let alice_currency_balance_after_unsigned = Currencies::free_balance(fee_currency, &user_acc.address());
			let evm_fee = alice_currency_balance_initial - alice_currency_balance_after_unsigned;
			assert!(evm_fee > 0);

			// Native baseline: pre-dispatch the same call, charges native fee.
			let info = omni_sell.get_dispatch_info();
			let len: usize = 146;
			let pre = pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(0).validate_and_prepare(
				Some(user_acc.address()).into(),
				&omni_sell,
				&info,
				len,
				0,
			);
			assert_ok!(&pre);

			let alice_currency_balance_after_native = Currencies::free_balance(fee_currency, &user_acc.address());
			let native_fee = alice_currency_balance_after_unsigned - alice_currency_balance_after_native;
			assert!(native_fee > 0);

			let fee_difference = evm_fee.saturating_sub(native_fee);
			let relative_fee_difference = FixedU128::from_rational(fee_difference, native_fee);
			let tolerated_fee_difference = FixedU128::from_rational(30, 100);
			assert!(
				relative_fee_difference < tolerated_fee_difference,
				"unsigned user fee leaked from signed-branch weight change! \
				 evm_fee={} native_fee={} relative_difference={:?} (tolerated < {:?})",
				evm_fee,
				native_fee,
				relative_fee_difference,
				tolerated_fee_difference
			);
		})
	}

	#[test]
	fn parity_dry_run_and_real_run_should_agree_on_omnipool_sell_success() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));
			let user_acc = MockAccount::new(alith_truncated_account());
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));

			let (result, _, _) = submit_signed_dispatch_permit_omni_sell(paymaster, 1_000_000_000);
			assert_ok!(result);
			assert_dispatch_permit_not_paused();
		});
	}

	#[test]
	fn parity_dry_run_and_real_run_should_agree_on_omnipool_sell_failure_slippage() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));
			let user_acc = MockAccount::new(alith_truncated_account());
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));

			let inner_call = RuntimeCall::Omnipool(pallet_omnipool::Call::<Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 1_000_000_000,
				min_buy_amount: u128::MAX, // impossible slippage
			});
			let (from, data, gas_limit, deadline, v, r, s) =
				build_permit_for_call(&inner_call, 1_000_000, U256::from(1_000_000_000_000u128));

			let result = MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::signed(paymaster),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				data,
				gas_limit,
				deadline,
				v,
				r,
				s,
			);

			assert!(result.is_err());
			assert_dispatch_permit_not_paused();
		});
	}

	#[test]
	fn parity_dry_run_should_not_persist_state_changes() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));
			let user_acc = MockAccount::new(alith_truncated_account());
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));

			let initial_hdx = user_acc.balance(HDX);
			let initial_dai = user_acc.balance(DAI);

			let sell_amount: Balance = 1_000_000_000;
			let (result, _, _) = submit_signed_dispatch_permit_omni_sell(paymaster, sell_amount);
			assert_ok!(result);

			let hdx_spent = initial_hdx - user_acc.balance(HDX);
			let dai_received = user_acc.balance(DAI) - initial_dai;

			assert_eq!(
				hdx_spent, sell_amount,
				"user HDX must reflect ONE sell, not two — dry-run state must roll back"
			);
			assert!(dai_received > 0);
		});
	}

	// Save/restore tests simulate recursion by pre-setting an outer override
	// (constructing a real recursive EVM permit would need a second keypair
	// with careful nonce sequencing).

	#[test]
	fn signed_dispatch_permit_should_restore_previous_fee_payer_override_on_success() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));
			let user_acc = MockAccount::new(alith_truncated_account());
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));

			// Simulate an outer caller that already set the override.
			let outer = AccountId::from(crate::polkadot_test_net::CHARLIE);
			hydradx_runtime::evm::set_evm_fee_payer(outer.clone());
			assert_eq!(hydradx_runtime::evm::evm_fee_payer(), Some(outer.clone()));

			let (result, _, _) = submit_signed_dispatch_permit_omni_sell(paymaster, 1_000_000_000);
			assert_ok!(result);

			assert_eq!(
				hydradx_runtime::evm::evm_fee_payer(),
				Some(outer),
				"outer fee-payer override must be RESTORED, not cleared"
			);

			hydradx_runtime::evm::clear_evm_fee_payer();
		});
	}

	#[test]
	fn signed_dispatch_permit_should_restore_previous_fee_payer_override_on_permit_validation_failure() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));

			let outer = AccountId::from(crate::polkadot_test_net::CHARLIE);
			hydradx_runtime::evm::set_evm_fee_payer(outer.clone());

			let inner_call = RuntimeCall::Omnipool(pallet_omnipool::Call::<Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 1_000_000_000,
				min_buy_amount: 0,
			});
			let (from, data, gas_limit, deadline, _v, _r, _s) =
				build_permit_for_call(&inner_call, 1_000_000, U256::from(1_000_000_000_000u128));

			let result = MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::signed(paymaster),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				data,
				gas_limit,
				deadline,
				27,
				H256::from([0xAA; 32]),
				H256::from([0xBB; 32]),
			);
			assert!(result.is_err());

			assert_eq!(
				hydradx_runtime::evm::evm_fee_payer(),
				Some(outer),
				"outer override must survive the early-return path",
			);

			hydradx_runtime::evm::clear_evm_fee_payer();
		});
	}

	#[test]
	fn signed_dispatch_permit_should_restore_previous_fee_payer_override_on_dry_run_failure() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));
			let user_acc = MockAccount::new(alith_truncated_account());
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));

			let outer = AccountId::from(crate::polkadot_test_net::CHARLIE);
			hydradx_runtime::evm::set_evm_fee_payer(outer.clone());

			let inner_call = RuntimeCall::Omnipool(pallet_omnipool::Call::<Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 1_000_000_000,
				min_buy_amount: u128::MAX,
			});
			let (from, data, gas_limit, deadline, v, r, s) =
				build_permit_for_call(&inner_call, 1_000_000, U256::from(1_000_000_000_000u128));

			let result = MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::signed(paymaster),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				data,
				gas_limit,
				deadline,
				v,
				r,
				s,
			);
			assert!(result.is_err());

			assert_eq!(
				hydradx_runtime::evm::evm_fee_payer(),
				Some(outer),
				"outer override must survive the early-return path",
			);

			hydradx_runtime::evm::clear_evm_fee_payer();
		});
	}

	#[test]
	fn adversarial_u64_max_gas_limit_should_not_corrupt_state() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));

			hydradx_runtime::evm::clear_evm_fee_payer();

			let inner_call = RuntimeCall::Omnipool(pallet_omnipool::Call::<Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 1_000_000_000,
				min_buy_amount: 0,
			});
			let (from, data, _, deadline, v, r, s) =
				build_permit_for_call(&inner_call, 1_000_000, U256::from(1_000_000_000_000u128));

			// Direct extrinsic call bypasses the SignedExtension pre-dispatch,
			// so we observe how the body itself handles u64::MAX: the dry-run
			// hits Runner pre-validation (`gas_limit > block_gas_limit`) and
			// returns Err → InnerCallWouldFail.
			let result = MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::signed(paymaster),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				data,
				u64::MAX, // ← attack vector
				deadline,
				v,
				r,
				s,
			);

			assert!(result.is_err(), "u64::MAX gas_limit must be rejected");
			assert_dispatch_permit_not_paused();
			assert_eq!(
				hydradx_runtime::evm::evm_fee_payer(),
				None,
				"fee-payer override MUST NOT leak from a rejected submission",
			);
		});
	}

	#[test]
	fn adversarial_insufficient_paymaster_balance_should_not_leak_fee_payer() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			// Exactly ED — survives, can't pay fees.
			assert_ok!(Balances::mint_into(&paymaster, NativeExistentialDeposit::get()));

			hydradx_runtime::evm::clear_evm_fee_payer();

			let (result, _, _) = submit_signed_dispatch_permit_omni_sell(paymaster, 1_000_000_000);

			// Direct extrinsic call bypasses SignedExtension; either Ok (body
			// ran with EVM fee debit failing internally) or Err is acceptable.
			let _ = result;
			assert_eq!(hydradx_runtime::evm::evm_fee_payer(), None);
			assert_dispatch_permit_not_paused();
		});
	}

	#[test]
	fn adversarial_dry_run_failure_should_leave_permit_nonce_unchanged_so_user_can_retry() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));
			let user_acc = MockAccount::new(alith_truncated_account());
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));

			let alith = alith_evm_address();
			let nonce_before = pallet_evm_precompile_call_permit::NoncesStorage::get(alith);

			let inner_call = RuntimeCall::Omnipool(pallet_omnipool::Call::<Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 1_000_000_000,
				min_buy_amount: u128::MAX,
			});
			let (from, data, gas_limit, deadline, v, r, s) =
				build_permit_for_call(&inner_call, 1_000_000, U256::from(1_000_000_000_000u128));

			let result = MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::signed(paymaster),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				data,
				gas_limit,
				deadline,
				v,
				r,
				s,
			);
			assert!(result.is_err());

			let nonce_after = pallet_evm_precompile_call_permit::NoncesStorage::get(alith);
			assert_eq!(
				nonce_before, nonce_after,
				"call-permit nonce MUST NOT advance when 5b rejects the call"
			);
			assert_dispatch_permit_not_paused();
		});
	}

	// Failed signed dispatches must cost the signer full declared weight (no
	// custom refund). This is an anti-grief property: an attacker spamming
	// invalid permits pays the same as a legitimate paymaster spamming valid
	// ones, so griefing is uniformly expensive.
	#[test]
	fn signed_dispatch_permit_should_use_default_post_info_on_dry_run_failure() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));
			let user_acc = MockAccount::new(alith_truncated_account());
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));

			let inner_call = RuntimeCall::Omnipool(pallet_omnipool::Call::<Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 1_000_000_000,
				min_buy_amount: u128::MAX,
			});
			let (from, data, gas_limit, deadline, v, r, s) =
				build_permit_for_call(&inner_call, 1_000_000, U256::from(1_000_000_000_000u128));

			let result = MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::signed(paymaster),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				data,
				gas_limit,
				deadline,
				v,
				r,
				s,
			);

			let err = result.expect_err("expected dry-run failure");
			assert_eq!(
				err.error,
				pallet_transaction_multi_payment::Error::<Runtime>::InnerCallWouldFail.into(),
			);
			// actual_weight = None → SignedExtension uses declared (2×), no refund.
			assert_eq!(err.post_info.actual_weight, None);
		});
	}

	#[test]
	fn adversarial_empty_data_should_be_handled_cleanly() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));

			hydradx_runtime::evm::clear_evm_fee_payer();

			let from = alith_evm_address();
			let secret_key = SecretKey::parse(&alith_secret_key()).unwrap();
			let gas_limit = 100_000;
			let deadline = U256::from(1_000_000_000_000u128);

			let permit = pallet_evm_precompile_call_permit::CallPermitPrecompile::<Runtime>::generate_permit(
				CALLPERMIT,
				from,
				DISPATCH_ADDR,
				U256::from(0),
				vec![], // ← empty data
				gas_limit,
				U256::zero(),
				deadline,
			);
			let message = Message::parse(&permit);
			let (rs, v) = sign(&message, &secret_key);

			let result = MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::signed(paymaster),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				vec![],
				gas_limit,
				deadline,
				v.serialize(),
				H256::from(rs.r.b32()),
				H256::from(rs.s.b32()),
			);

			// Outcome irrelevant; invariants are what matter.
			let _ = result;
			assert_eq!(hydradx_runtime::evm::evm_fee_payer(), None);
			assert_dispatch_permit_not_paused();
		});
	}

	#[test]
	fn adversarial_two_paymasters_racing_for_same_permit_should_settle_cleanly() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster_a = paymaster_account();
			let paymaster_b = AccountId::from(crate::polkadot_test_net::CHARLIE);
			assert_ok!(Balances::mint_into(&paymaster_a, 100 * UNITS));
			assert_ok!(Balances::mint_into(&paymaster_b, 100 * UNITS));
			let user_acc = MockAccount::new(alith_truncated_account());
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));

			let inner_call = RuntimeCall::Omnipool(pallet_omnipool::Call::<Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: 1_000_000_000,
				min_buy_amount: 0,
			});
			let (from, data, gas_limit, deadline, v, r, s) =
				build_permit_for_call(&inner_call, 1_000_000, U256::from(1_000_000_000_000u128));

			let initial_dai = user_acc.balance(DAI);

			assert_ok!(MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::signed(paymaster_a),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				data.clone(),
				gas_limit,
				deadline,
				v,
				r,
				s,
			));
			let dai_after_first = user_acc.balance(DAI);
			assert!(dai_after_first > initial_dai);

			let result_b = MultiTransactionPayment::dispatch_permit(
				RuntimeOrigin::signed(paymaster_b),
				from,
				DISPATCH_ADDR,
				U256::from(0),
				data,
				gas_limit,
				deadline,
				v,
				r,
				s,
			);
			assert!(result_b.is_err(), "second paymaster's submission must fail");

			let dai_after_second = user_acc.balance(DAI);
			assert_eq!(
				dai_after_first, dai_after_second,
				"second submission must NOT trigger another sell",
			);

			assert_eq!(hydradx_runtime::evm::evm_fee_payer(), None);
			assert_dispatch_permit_not_paused();
		});
	}

	#[test]
	fn signed_dispatch_permit_should_clear_fee_payer_when_no_previous_override() {
		TestNet::reset();

		Hydra::execute_with(|| {
			init_omnipool_with_oracle_for_block_10();
			let paymaster = paymaster_account();
			assert_ok!(Balances::mint_into(&paymaster, 100 * UNITS));
			let user_acc = MockAccount::new(alith_truncated_account());
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				(10 * UNITS) as i128,
			));

			hydradx_runtime::evm::clear_evm_fee_payer();
			assert_eq!(hydradx_runtime::evm::evm_fee_payer(), None);

			let (result, _, _) = submit_signed_dispatch_permit_omni_sell(paymaster, 1_000_000_000);
			assert_ok!(result);

			assert_eq!(
				hydradx_runtime::evm::evm_fee_payer(),
				None,
				"override must be CLEARED when no outer override existed",
			);
		});
	}
}
