use crate::evm::MockHandle;
use crate::polkadot_test_net::*;
use crate::utils::contracts::*;
use core::panic;
use ethabi::ethereum_types::BigEndianHash;
use fp_evm::ExitReason::Succeed;
use fp_evm::PrecompileSet;
use frame_support::pallet_prelude::DispatchError::Other;
use frame_support::storage::with_transaction;
use frame_support::{assert_noop, assert_ok};
use hydradx_runtime::evm::precompiles::HydraDXPrecompiles;
use hydradx_runtime::evm::{Erc20Currency, EvmNonceProvider as AccountNonce, Executor, Function};
use hydradx_runtime::AssetRegistry;
use hydradx_runtime::RuntimeCall;
use hydradx_runtime::RuntimeOrigin;
use hydradx_runtime::{AssetLocation, Currencies};
use hydradx_runtime::{EVMAccounts, Runtime};
use hydradx_traits::evm::ERC20;
use hydradx_traits::evm::EVM;
use hydradx_traits::evm::{CallContext, EvmAddress};
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use orml_traits::MultiCurrency;
use pallet_evm::ExitSucceed::Returned;
use pallet_evm_accounts::EvmNonceProvider;
use polkadot_xcm::v3::Junction::AccountKey20;
use polkadot_xcm::v3::Junctions::X1;
use polkadot_xcm::v3::MultiLocation;
use primitives::AccountId;
use scraper::{ALICE, BOB};
use sp_core::keccak_256;
use sp_core::Encode;
use sp_core::{H256, U256};
use sp_runtime::{Permill, TransactionOutcome};
use std::fmt::Write;
use xcm_emulator::TestExt;

pub fn deployer() -> EvmAddress {
	EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE))
}

pub fn deploy_token_contract() -> EvmAddress {
	deploy_contract("HydraToken", deployer())
}

pub fn bind_erc20(contract: EvmAddress) -> AssetId {
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
fn executor_view_should_return_something() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let token = deploy_token_contract();

		let data = Into::<u32>::into(Function::TotalSupply).to_be_bytes().to_vec();
		let context = CallContext {
			contract: token,
			sender: Default::default(),
			origin: Default::default(),
		};
		let (res, value) = Executor::<Runtime>::view(context, data, 100_000);

		assert_eq!(res, Succeed(Returned));
		assert_ne!(value, vec![0; value.len()]);
	});
}

#[test]
fn executor_call_wont_bump_nonce() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let token = deploy_token_contract();

		let nonce = AccountNonce::get_nonce(deployer());

		let mut data = Into::<u32>::into(Function::Transfer).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(evm_address()).as_bytes());
		data.extend_from_slice(H256::from_uint(&U256::from(100)).as_bytes());
		let context = CallContext {
			contract: token,
			sender: deployer(),
			origin: deployer(),
		};
		let (res, value) = Executor::<Runtime>::call(context, data, U256::zero(), 100_000);

		assert_eq!(res, Succeed(Returned));
		assert_ne!(value, vec![0; value.len()]);

		assert_eq!(AccountNonce::get_nonce(deployer()), nonce);
	});
}

#[test]
fn name_should_decode_correctly() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let token = deploy_token_contract();

		assert_eq!(
			Erc20Currency::<Runtime>::name(CallContext::new_view(token)),
			Some("Hydra".as_bytes().to_vec())
		);
	});
}

#[test]
fn total_issuance_should_decode_correctly() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let token = deploy_token_contract();

		assert_eq!(
			Erc20Currency::<Runtime>::total_issuance(token),
			1_000_000_000 * 10u128.pow(18)
		);
	});
}

#[test]
fn total_supply_should_decode_correctly() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let token = deploy_token_contract();

		assert_eq!(
			Erc20Currency::<Runtime>::total_supply(CallContext::new_view(token)),
			1_000_000_000 * 10u128.pow(18)
		);
	});
}

#[test]
fn symbol_should_decode_correctly() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let token = deploy_token_contract();

		assert_eq!(
			Erc20Currency::<Runtime>::symbol(CallContext::new_view(token)),
			Some("HYDRA".as_bytes().to_vec())
		);
	});
}

#[test]
fn decimals_should_decode_correctly() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let token = deploy_token_contract();

		assert_eq!(
			Erc20Currency::<Runtime>::decimals(CallContext::new_view(token)),
			Some(18)
		);
	});
}

#[test]
fn deployer_should_have_balance() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let token = deploy_token_contract();

		assert_eq!(
			Erc20Currency::<Runtime>::balance_of(CallContext::new_view(token), deployer()),
			1_000_000_000 * 10u128.pow(18)
		);
	});
}

#[test]
fn address_should_receive_tokens() {
	TestNet::reset();
	Hydra::execute_with(|| {
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

		assert_eq!(
			Erc20Currency::<Runtime>::balance_of(CallContext::new_view(token), evm_address()),
			100
		);
	});
}

#[test]
fn approve_should_increase_allowance() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let token = deploy_token_contract();

		assert_ok!(<Erc20Currency<Runtime> as ERC20>::approve(
			CallContext {
				contract: token,
				sender: deployer(),
				origin: deployer()
			},
			evm_address(),
			100
		));

		assert_eq!(
			Erc20Currency::<Runtime>::allowance(CallContext::new_view(token), deployer(), evm_address()),
			100
		);
	});
}

#[test]
fn transfer_from_can_spend_allowance() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let token = deploy_token_contract();

		assert_ok!(<Erc20Currency<Runtime> as ERC20>::approve(
			CallContext::new_call(token, deployer()),
			evm_address(),
			100
		));
		assert_ok!(<Erc20Currency<Runtime> as ERC20>::transfer_from(
			CallContext::new_call(token, evm_address()),
			deployer(),
			evm_address(),
			100
		));

		assert_eq!(
			Erc20Currency::<Runtime>::balance_of(CallContext::new_view(token), evm_address()),
			100
		);
		assert_eq!(
			Erc20Currency::<Runtime>::allowance(CallContext::new_view(token), deployer(), evm_address()),
			0
		);
	});
}

#[test]
fn alice_should_have_free_balance() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let token = deploy_token_contract();

		assert_eq!(
			Erc20Currency::<Runtime>::free_balance(token, &AccountId::from(ALICE)),
			1_000_000_000 * 10u128.pow(18)
		);
	});
}

#[test]
fn account_should_receive_tokens() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let token = deploy_token_contract();

		assert_ok!(<Erc20Currency<Runtime> as MultiCurrency<AccountId>>::transfer(
			token,
			&AccountId::from(ALICE),
			&AccountId::from(BOB),
			100
		));

		assert_eq!(
			Erc20Currency::<Runtime>::free_balance(token, &AccountId::from(BOB)),
			100
		);
	});
}

#[test]
fn bound_erc20_should_have_issuance() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_token_contract();
		let asset = bind_erc20(contract);

		assert_eq!(Currencies::total_issuance(asset), 1_000_000_000 * 10u128.pow(18));
	});
}

#[test]
fn currencies_should_transfer_bound_erc20() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_token_contract();
		let asset = bind_erc20(contract);

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			BOB.into(),
			asset,
			100
		));

		assert_eq!(Currencies::free_balance(asset, &BOB.into()), 100);
		assert_eq!(Erc20Currency::<Runtime>::free_balance(contract, &BOB.into()), 100);
	});
}

fn error_signature(definition: &str) -> String {
	let hash = keccak_256(definition.as_bytes());
	hash[..4].iter().fold(String::new(), |mut acc, b| {
		write!(&mut acc, "{:02x}", b).unwrap();
		acc
	})
}

#[test]
fn insufficient_balance_should_fail_transfer() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_token_contract();
		let asset = bind_erc20(contract);

		match Currencies::transfer(RuntimeOrigin::signed(BOB.into()), ALICE.into(), asset, 100) {
			Err(Other(e)) => {
				assert!(e.contains(&error_signature("ERC20InsufficientBalance(address,uint256,uint256)")));
			}
			_ => panic!("transfer should fail"),
		}
	});
}

#[test]
fn deposit_is_not_supported() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_token_contract();
		let asset = bind_erc20(contract);

		assert_noop!(
			Currencies::deposit(asset, &ALICE.into(), 100),
			pallet_currencies::Error::<Runtime>::NotSupported
		);
	});
}

#[test]
fn withdraw_is_not_supported() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_token_contract();
		let asset = bind_erc20(contract);

		assert_noop!(
			Currencies::withdraw(asset, &ALICE.into(), 100),
			pallet_currencies::Error::<Runtime>::NotSupported
		);
	});
}

#[test]
fn erc20_currency_is_tradeable_in_omnipool() {
	TestNet::reset();
	Hydra::execute_with(|| {
		init_omnipool();
		let contract = deploy_token_contract();
		let erc20 = bind_erc20(contract);
		let amount = 1_000_000_000_000_000;
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			pallet_omnipool::Pallet::<Runtime>::protocol_account(),
			erc20,
			amount,
		));
		assert_ok!(pallet_omnipool::Pallet::<Runtime>::add_token(
			RuntimeOrigin::root(),
			erc20,
			45_000_000_000.into(),
			Permill::from_percent(30),
			ALICE.into(),
		));

		assert_ok!(pallet_route_executor::Pallet::<Runtime>::sell(
			RuntimeOrigin::signed(ALICE.into()),
			erc20,
			DAI,
			1_000,
			1,
			vec![],
		));
	});
}

#[test]
fn erc20_currency_transfer_should_be_callable_using_dispatch_precompile() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let contract = deploy_token_contract();
		let erc20 = bind_erc20(contract);
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			evm_account(),
			erc20,
			1000,
		));

		// Act
		let call = RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: BOB.into(),
			currency_id: erc20,
			amount: 100,
		});
		let prec = HydraDXPrecompiles::<hydradx_runtime::Runtime>::new();
		assert_ok!(prec
			.execute(&mut MockHandle::new_dispatch(evm_address(), call.encode()))
			.unwrap());

		//Assert
		assert_eq!(Currencies::free_balance(erc20, &BOB.into()), 100);
	});
}
