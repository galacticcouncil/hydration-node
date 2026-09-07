#![cfg(test)]

use crate::polkadot_test_net::*;
use crate::utils::accounts::*;
use ethabi::ethereum_types::BigEndianHash;
use hydradx_runtime::evm::{Erc20Currency, Executor, Function};

use crate::utils::contracts::deploy_contract;
use crate::utils::executive::assert_executive_apply_signed_extrinsic;
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
use hydradx_runtime::DOT_ASSET_LOCATION;
use hydradx_runtime::XYK;
use hydradx_runtime::{AssetLocation, EVMAccounts, System};
use hydradx_runtime::{AssetRegistry, TreasuryAccount};
use hydradx_runtime::{
	Balances, Currencies, DotAssetId, MultiTransactionPayment, Omnipool, RuntimeCall, RuntimeOrigin, Tokens,
	XykPaymentAssetSupport,
};
use hydradx_runtime::{FixedU128, Runtime};
use hydradx_traits::evm::ERC20;
use hydradx_traits::evm::{CallContext, EVM};
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use hydradx_traits::Mutate as AssetRegistryMutate;
use libsecp256k1::{sign, Message, SecretKey};
use orml_traits::MultiCurrency;
use pallet_evm_accounts::EvmNonceProvider;
use pallet_transaction_multi_payment::EVMPermit;
use polkadot_xcm::v3::Junction::AccountKey20;
use polkadot_xcm::v3::Junctions::X1;
use polkadot_xcm::v3::MultiLocation;
use pretty_assertions::assert_eq;
use primitives::constants::currency::UNITS;
use primitives::{AssetId, Balance, EvmAddress};
use sp_core::{Pair, H256, U256};
use sp_runtime::traits::SignedExtension;
use sp_runtime::traits::{Convert, IdentifyAccount};
use sp_runtime::transaction_validity::InvalidTransaction;
use sp_runtime::transaction_validity::TransactionValidityError;
use sp_runtime::transaction_validity::{TransactionSource, ValidTransaction};
use sp_runtime::Permill;
use sp_runtime::TransactionOutcome;
use sp_runtime::{DispatchResult, SaturatedConversion};
use xcm_emulator::TestExt;

pub const TREASURY_ACCOUNT_INIT_BALANCE: Balance = 1000 * UNITS;

pub const PATH_TO_SNAPSHOT: &str = "snapshots/hsm/mainnet_nov";

fn test_user_evm_account() -> EvmAddress {
	alith_evm_address()
}

fn test_user_new_account() -> MockAccount {
	MockAccount::new(alith_evm_account())
}

fn treasury_account() -> MockAccount {
	MockAccount::new(Treasury::account_id())
}

fn deployer() -> hydradx_traits::evm::EvmAddress {
	EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE))
}

fn deploy_token_contract() -> hydradx_traits::evm::EvmAddress {
	deploy_contract("HydraToken", crate::erc20::deployer())
}

pub fn bind_erc20(contract: hydradx_traits::evm::EvmAddress) -> AssetId {
	let token = CallContext::new_view(contract);
	let asset = with_transaction(|| {
		TransactionOutcome::Commit(AssetRegistry::register_sufficient_asset(
			None,
			Some(Erc20Currency::<Runtime>::name(token).unwrap().try_into().unwrap()),
			AssetKind::Erc20,
			1,
			Some(Erc20Currency::<Runtime>::symbol(token).unwrap().try_into().unwrap()),
			Some(Erc20Currency::<Runtime>::decimals(token).unwrap()),
			Some(AssetLocation(MultiLocation::new(
				0,
				X1(AccountKey20 {
					key: contract.into(),
					network: None,
				}),
			))),
			None,
		))
	});
	asset.unwrap()
}

#[test]
fn address_should_have_increased_providers_when_receive_erco20() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let user_acc = MockAccount::new(evm_account());

		let token = deploy_token_contract();

		assert_ok!(<Erc20Currency<Runtime> as ERC20>::transfer(
			CallContext {
				contract: token,
				sender: deployer(),
				origin: deployer()
			},
			evm_address(),
			100
		));

		std::assert_eq!(
			Erc20Currency::<Runtime>::balance_of(CallContext::new_view(token), evm_address()),
			100
		);
		assert!(frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
	});
}

#[test]
fn haha() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let user_acc = MockAccount::new(evm_account());

		let token = deploy_token_contract();
		let context = CallContext {
			contract: token,
			sender: deployer(),
			origin: deployer(),
		};

		let mut data = Into::<u32>::into(Function::Transfer).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(evm_address()).as_bytes());
		data.extend_from_slice(H256::from_uint(&U256::from(100u128.saturated_into::<u128>())).as_bytes());

		let r = Executor::<Runtime>::call(context, data, U256::zero(), 400_000);

		assert_eq!(
			Erc20Currency::<Runtime>::balance_of(CallContext::new_view(token), evm_address()),
			100
		);
		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			1_000_000_000_000_000_i128,
		));

		dbg!(user_acc.balance(HDX));

		assert!(frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);

		assert_ok!(<Erc20Currency<Runtime> as ERC20>::transfer(
			CallContext {
				contract: token,
				sender: evm_address(),
				origin: evm_address()
			},
			deployer(),
			100
		));

		std::assert_eq!(
			Erc20Currency::<Runtime>::balance_of(CallContext::new_view(token), evm_address()),
			0
		);
		assert!(frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		dbg!(user_acc.balance(HDX));

		//let info = user_acc.account_info();
		//assert_eq!(info.providers, 1);

		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			user_acc.address(),
			HDX,
			1_000_000_000_000_000_i128,
		));

		dbg!(user_acc.balance(HDX));
	});
}

#[test]
fn account_providers_should_increase_when_transferring_native_asset_to_new_account() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let user_acc = test_user_new_account();
		let treasury_acc = treasury_account();

		assert!(!frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_acc.address()),
			user_acc.address().into(),
			0,
			1_000_000_000_000_000,
		));

		assert!(frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
	});
}

#[test]
fn account_providers_should_increase_when_transferring_nonnative_asset_to_new_account() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let user_acc = test_user_new_account();
		let treasury_acc = treasury_account();

		assert!(!frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_acc.address()),
			user_acc.address().into(),
			1,
			1_000_000_000_000_000,
		));

		assert!(frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
	});
}

#[test]
fn account_providers_should_increase_when_transferring_erc20_asset_to_new_account() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let user_acc = test_user_new_account();
		let treasury_acc = treasury_account();

		assert!(!frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_acc.address()),
			user_acc.address().into(),
			222,
			1_000_000_000_000_000_000_000,
		));

		assert!(
			frame_system::Account::<hydradx_runtime::Runtime>::contains_key(user_acc.address()),
			"New account with balance not found in the system"
		);
		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
	});
}

#[test]
fn account_providers_should_increase_for_each_new_asset() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let user_acc = test_user_new_account();
		let treasury_acc = treasury_account();

		assert!(!frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_acc.address()),
			user_acc.address().into(),
			0,
			1_000_000_000_000_000,
		));
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_acc.address()),
			user_acc.address().into(),
			20,
			1_000_000_000_000_000,
		));
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_acc.address()),
			user_acc.address().into(),
			1,
			1_000_000_000_000_000,
		));

		assert!(frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		let info = user_acc.account_info();
		assert_eq!(info.providers, 3);
	});
}

#[test]
fn removing_all_but_erc20_should_not_lock_you_out() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let user_acc = test_user_new_account();
		let treasury_acc = treasury_account();

		assert!(!frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_acc.address()),
			user_acc.address().into(),
			0,
			1_000_000_000_000_000,
		));
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_acc.address()),
			user_acc.address().into(),
			222,
			1_000_000_000_000_000_000_000,
		));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(user_acc.address()),
			treasury_acc.address().into(),
			0,
			1_000_000_000_000_000,
		));

		assert!(frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		let info = user_acc.account_info();
		dbg!(&info);
		assert_eq!(info.providers, 1);
	});
}

#[test]
fn account_nonce_should_correctly_increase_when_signing_transaction() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let (pair, _) = sp_core::sr25519::Pair::generate();
		let user_acc = MockAccount::new(sp_runtime::MultiSigner::from(pair.public()).into_account());
		let treasury_acc = treasury_account();
		hydradx_run_to_next_block();

		// Send some HDX for fee
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_acc.address()),
			user_acc.address().into(),
			0,
			1_000_000_000_000_000,
		));

		assert!(frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
		assert_eq!(info.nonce, 0);

		let remark = hydradx_runtime::RuntimeCall::System(frame_system::Call::remark { remark: vec![] });

		let _ = assert_executive_apply_signed_extrinsic(remark, pair);

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
		assert_eq!(info.nonce, 1);
	});
}

#[test]
fn account_nonce_should_correctly_increase_when_signing_transaction_with_nonnative_currency() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let (pair, _) = sp_core::sr25519::Pair::generate();
		let user_acc = MockAccount::new(sp_runtime::MultiSigner::from(pair.public()).into_account());
		let treasury_acc = treasury_account();
		hydradx_run_to_next_block();

		// Send some HDX for fee
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_acc.address()),
			user_acc.address().into(),
			20,
			100_000_000_000_000_000,
		));

		assert!(frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
		assert_eq!(info.nonce, 0);

		let payment_asset = pallet_transaction_multi_payment::AccountCurrencyMap::<Runtime>::get(user_acc.address());
		dbg!(payment_asset);
		assert_eq!(payment_asset, Some(WETH));

		let remark = hydradx_runtime::RuntimeCall::System(frame_system::Call::remark { remark: vec![] });

		let r = assert_executive_apply_signed_extrinsic(remark, pair);
		dbg!(r);

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
		assert_eq!(info.nonce, 1);
	});
}

#[test]
fn account_nonce_should_correctly_increase_when_signing_transaction_with_erc20_currrency() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let (pair, _) = sp_core::sr25519::Pair::generate();
		let user_acc = MockAccount::new(sp_runtime::MultiSigner::from(pair.public()).into_account());
		let treasury_acc = treasury_account();
		hydradx_run_to_next_block();

		// Send some HDX for fee
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_acc.address()),
			user_acc.address().into(),
			222,
			1_000_000_000_000_000_000,
		));

		/*
		assert!(frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
		assert_eq!(info.nonce, 0);

		let payment_asset = pallet_transaction_multi_payment::AccountCurrencyMap::<Runtime>::get(user_acc.address());
		dbg!(payment_asset);
		assert_eq!(payment_asset, Some(WETH));

		 */

		let remark = hydradx_runtime::RuntimeCall::System(frame_system::Call::remark { remark: vec![] });

		let r = assert_executive_apply_signed_extrinsic(remark, pair);
		dbg!(r);

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
		assert_eq!(info.nonce, 1);
	});
}

#[test]
fn account_with_erc20_only_should_work() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let (pair, _) = sp_core::sr25519::Pair::generate();
		let user_acc = MockAccount::new(sp_runtime::MultiSigner::from(pair.public()).into_account());
		let treasury_acc = treasury_account();
		hydradx_run_to_next_block();

		// Send some HDX for fee
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_acc.address()),
			user_acc.address().into(),
			222,
			1_000_000_000_000_000_000,
		));

		/*
		assert!(frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
		assert_eq!(info.nonce, 0);

		let payment_asset = pallet_transaction_multi_payment::AccountCurrencyMap::<Runtime>::get(user_acc.address());
		dbg!(payment_asset);
		assert_eq!(payment_asset, Some(WETH));

		 */

		let call = RuntimeCall::MultiTransactionPayment(pallet_transaction_multi_payment::Call::set_currency {
			currency: 222,
		});

		let r = assert_executive_apply_signed_extrinsic(call, pair);
		dbg!(r);

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
		assert_eq!(info.nonce, 1);
	});
}

#[test]
fn account_with_erc20_and_hdx_should_work() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let (pair, _) = sp_core::sr25519::Pair::generate();
		let user_acc = MockAccount::new(sp_runtime::MultiSigner::from(pair.public()).into_account());
		let treasury_acc = treasury_account();
		hydradx_run_to_next_block();

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_acc.address()),
			user_acc.address().into(),
			0,
			1_000_000_000_000,
		));

		// Send some HDX for fee
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_acc.address()),
			user_acc.address().into(),
			222,
			1_000_000_000_000_000_000,
		));

		/*
		assert!(frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
		assert_eq!(info.nonce, 0);

		let payment_asset = pallet_transaction_multi_payment::AccountCurrencyMap::<Runtime>::get(user_acc.address());
		dbg!(payment_asset);
		assert_eq!(payment_asset, Some(WETH));

		 */

		let call = RuntimeCall::MultiTransactionPayment(pallet_transaction_multi_payment::Call::set_currency {
			currency: 222,
		});

		let r = assert_executive_apply_signed_extrinsic(call, pair);
		dbg!(r);

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
		assert_eq!(info.nonce, 1);

		dbg!(user_acc.balance(0));
		dbg!(user_acc.balance(222));
	});
}

#[test]
fn account_nonce_should_be_handled_correctly_during_permit_execution() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let (pair, _) = sp_core::sr25519::Pair::generate();
		let user_acc = MockAccount::new(sp_runtime::MultiSigner::from(pair.public()).into_account());
		let treasury_acc = treasury_account();
		hydradx_run_to_next_block();

		// Send some HDX for fee
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury_acc.address()),
			user_acc.address().into(),
			222,
			1_000_000_000_000_000_000,
		));

		/*
		assert!(frame_system::Account::<hydradx_runtime::Runtime>::contains_key(
			user_acc.address()
		));

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
		assert_eq!(info.nonce, 0);

		let payment_asset = pallet_transaction_multi_payment::AccountCurrencyMap::<Runtime>::get(user_acc.address());
		dbg!(payment_asset);
		assert_eq!(payment_asset, Some(WETH));

		 */

		let remark = hydradx_runtime::RuntimeCall::System(frame_system::Call::remark { remark: vec![] });

		let r = assert_executive_apply_signed_extrinsic(remark, pair);
		dbg!(r);

		let info = user_acc.account_info();
		assert_eq!(info.providers, 1);
		assert_eq!(info.nonce, 1);
	});
}
