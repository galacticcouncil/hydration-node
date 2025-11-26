use crate::evm::MockHandle;
use crate::polkadot_test_net::*;
use crate::utils::contracts::*;
use core::panic;
use frame_support::dispatch::GetDispatchInfo;
use pallet_transaction_payment::ChargeTransactionPayment;
use sp_core::crypto::Ss58Codec;
use sp_runtime::traits::SignedExtension;

use ethabi::ethereum_types::BigEndianHash;
use fp_evm::ExitReason::Succeed;
use fp_evm::PrecompileSet;
use frame_support::pallet_prelude::DispatchError::Other;
use frame_support::storage::with_transaction;
use frame_support::traits::NamedReservableCurrency;
use frame_support::{assert_noop, assert_ok};
use hydradx_runtime::evm::precompiles::HydraDXPrecompiles;
use hydradx_runtime::evm::{Erc20Currency, EvmNonceProvider as AccountNonce, Executor, Function};
use hydradx_runtime::RuntimeCall;
use hydradx_runtime::RuntimeOrigin;
use hydradx_runtime::{AssetLocation, Currencies};
use hydradx_runtime::{AssetRegistry, Balances};
use hydradx_runtime::{EVMAccounts, Runtime};
use hydradx_traits::evm::ERC20;
use hydradx_traits::evm::EVM;
use hydradx_traits::evm::{CallContext, EvmAddress};
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use orml_traits::MultiCurrency;
use pallet_evm::ExitSucceed::Returned;
use pallet_evm_accounts::Call::bind_evm_address;
use sp_core::bounded_vec::BoundedVec;

use hex_literal::hex;
use pallet_evm_accounts::EvmNonceProvider;
use polkadot_xcm::v3::Junction::AccountKey20;
use polkadot_xcm::v3::Junctions::X1;
use polkadot_xcm::v3::MultiLocation;
use primitives::AccountId;
use sp_core::keccak_256;
use sp_core::Encode;
use sp_core::{H256, U256};
use sp_runtime::{FixedU128, Permill, TransactionOutcome};
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
fn native_evm_account_should_have_free_balance() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_token_contract();
		let asset = bind_erc20(contract);
		assert_ok!(<Erc20Currency<Runtime> as ERC20>::transfer(
			CallContext {
				contract,
				sender: deployer(),
				origin: deployer()
			},
			evm_address(),
			100
		));

		assert_eq!(
			Erc20Currency::<Runtime>::balance_of(CallContext::new_view(contract), evm_address()),
			100
		);
		assert_eq!(
			Erc20Currency::<Runtime>::free_balance(contract, &EVMAccounts::account_id(evm_address())),
			100
		);
		assert_eq!(
			Currencies::free_balance(asset, &EVMAccounts::account_id(evm_address())),
			100
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
fn erc20_transfer_returning_false_should_be_handled_as_error() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let token = deploy_contract("WeirdToken", deployer());

		let asset = bind_erc20(token);
		assert_noop!(
			Currencies::transfer(RuntimeOrigin::signed(ALICE.into()), BOB.into(), asset, 100),
			Other("evm: erc20 transfer returned false")
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

		let evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE));
		let truncated_address = EVMAccounts::truncated_account_id(evm_address);

		let original_nonce = frame_system::Pallet::<Runtime>::account_nonce(truncated_address.clone());

		let init_weth_balance = 10000000000000000u128;
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			truncated_address.clone(),
			WETH,
			init_weth_balance as i128
		));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			BOB.into(),
			asset,
			100
		));

		//Assert that amount is transferred
		assert_eq!(Currencies::free_balance(asset, &BOB.into()), 100);
		assert_eq!(Erc20Currency::<Runtime>::free_balance(contract, &BOB.into()), 100);

		//Assert that no extra fee charged within EVM execution
		let weth_balance_after = Currencies::free_balance(WETH, &truncated_address.clone());
		assert_eq!(init_weth_balance, weth_balance_after);

		//Assert that nonce has not been changed
		let nonce_after = frame_system::Pallet::<Runtime>::account_nonce(truncated_address.clone());
		assert_eq!(nonce_after, original_nonce);

		//Assert transfer events
		let mut data = [0u8; 32];
		data[31] = 100;
		expect_hydra_last_events(vec![
			pallet_evm::Event::Log {
				log: pallet_evm::Log {
					address: contract,
					topics: vec![
						H256::from(hex!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")),
						H256::from(hex!("0000000000000000000000000404040404040404040404040404040404040404")),
						H256::from(hex!("0000000000000000000000000505050505050505050505050505050505050505")),
					],
					data: data.to_vec(),
				},
			}
			.into(),
			pallet_currencies::Event::Transferred {
				currency_id: asset,
				from: ALICE.into(),
				to: BOB.into(),
				amount: 100,
			}
			.into(),
		]);
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
fn deposit_fails_when_unsufficient_funds_in_hold() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_token_contract();
		let asset = bind_erc20(contract);

		assert_eq!(
			Currencies::deposit(asset, &ALICE.into(), 100),
			Err(Other("evm:0xe450d38c000000000000000000000000ffffffffffffffffffffffffffffffffffffffff00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064"))
		);
	});
}

#[test]
fn withdraw() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_token_contract();
		let asset = bind_erc20(contract);

		assert_ok!(Currencies::withdraw(asset, &ALICE.into(), 100));
	});
}

#[test]
fn deposit() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_token_contract();
		let asset = bind_erc20(contract);
		assert_ok!(Currencies::withdraw(asset, &ALICE.into(), 100));

		assert_ok!(Currencies::deposit(asset, &BOB.into(), 100));
		assert_eq!(Currencies::free_balance(asset, &BOB.into()), 100);
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
			BoundedVec::new(),
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

#[test]
fn blank_new_account_signed_tx_should_be_valid_when_contains_only_erc20() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		let contract = deploy_token_contract();
		let erc20 = bind_erc20(contract);

		let new_account: AccountId = AccountId::from([0xAA; 32]);

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			new_account.clone(),
			erc20,
			1000,
		));

		let set_currency_call =
			hydradx_runtime::RuntimeCall::MultiTransactionPayment(pallet_transaction_multi_payment::Call::<
				hydradx_runtime::Runtime,
			>::set_currency {
				currency: erc20,
			});

		let info = set_currency_call.get_dispatch_info();
		let info_len = 146;

		let nonce = frame_system::Pallet::<Runtime>::account_nonce(&new_account);
		let check_nonce = frame_system::CheckNonce::<Runtime>::from(nonce);
		let nonce_validation_result = check_nonce.validate(&new_account, &set_currency_call, &info, info_len);
		assert!(nonce_validation_result.is_ok());
	});
}

//TODO: use bigger amounts
//TODO: use totaly balance

#[test]
fn transfer_should_increment_providers_on_new_account() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let contract = deploy_token_contract();
		let erc20 = bind_erc20(contract);

		let new_account: AccountId = AccountId::from([37u8; 32]);
		let account_pre = frame_system::Pallet::<Runtime>::account(&new_account);
		assert_eq!(account_pre.providers, 0);
		assert_eq!(account_pre.sufficients, 0);
		assert_eq!(Currencies::free_balance(erc20, &new_account), 0);

		// Act
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			new_account.clone(),
			erc20,
			100,
		));

		// Assert - ERC20s now use sufficients instead of providers
		let account_post = frame_system::Pallet::<Runtime>::account(&new_account);
		assert_eq!(account_post.providers, 0);
		assert_eq!(account_post.sufficients, 1); // Incremented for ERC20
		assert_eq!(Currencies::free_balance(erc20, &new_account), 100);

		// Assert that multiple transfers to same account don't increment sufficients again
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			new_account.clone(),
			erc20,
			50,
		));
		let account_after_second = frame_system::Pallet::<Runtime>::account(&new_account);
		assert_eq!(account_after_second.providers, 0);
		assert_eq!(account_after_second.sufficients, 1); // Still 1
		assert_eq!(Currencies::free_balance(erc20, &new_account), 150);
	});
}

#[test]
fn transfer_should_decrement_providers_when_balance_becomes_zero() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let contract = deploy_token_contract();
		let erc20 = bind_erc20(contract);

		let new_account: AccountId = AccountId::from([37u8; 32]);
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			new_account.clone(),
			erc20,
			100,
		));
		let account_pre = frame_system::Pallet::<Runtime>::account(&new_account);
		assert_eq!(account_pre.providers, 0);
		assert_eq!(account_pre.sufficients, 1); // ERC20 uses sufficients
		assert_eq!(Currencies::free_balance(erc20, &new_account), 100);

		// Act
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(new_account.clone()),
			ALICE.into(),
			erc20,
			100,
		));

		// Assert - sufficients decremented when balance reaches zero
		let account_post = frame_system::Pallet::<Runtime>::account(&new_account);
		assert_eq!(account_post.providers, 0);
		assert_eq!(account_post.sufficients, 0); // Decremented
		assert_eq!(Currencies::free_balance(erc20, &new_account), 0);
	});
}

#[test]
fn transfer_should_not_decrement_providers_when_partial_balance_remains() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let contract = deploy_token_contract();
		let erc20 = bind_erc20(contract);

		let new_account: AccountId = AccountId::from([37u8; 32]);
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			new_account.clone(),
			erc20,
			100,
		));
		let account_pre = frame_system::Pallet::<Runtime>::account(&new_account);
		assert_eq!(account_pre.providers, 0);
		assert_eq!(account_pre.sufficients, 1); // ERC20 uses sufficients

		// Act - transfer partial balance away from new account
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(new_account.clone()),
			ALICE.into(),
			erc20,
			50,
		));

		// Assert that sufficients remained the same
		let account_post = frame_system::Pallet::<Runtime>::account(&new_account);
		assert_eq!(account_post.providers, 0);
		assert_eq!(account_post.sufficients, 1); // Still 1
		assert_eq!(Currencies::free_balance(erc20, &new_account), 50);
	});
}

#[test]
fn deposit_should_increment_providers_on_new_account() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let contract = deploy_token_contract();
		let erc20 = bind_erc20(contract);

		let new_account: AccountId = AccountId::from([37u8; 32]);
		let account_pre = frame_system::Pallet::<Runtime>::account(&new_account);
		assert_eq!(account_pre.providers, 0);
		assert_eq!(account_pre.sufficients, 0);
		assert_eq!(Currencies::free_balance(erc20, &new_account), 0);

		// Withdraw some funds to holding account so deposit can be made
		assert_ok!(Currencies::withdraw(erc20, &ALICE.into(), 100));

		// Act
		assert_ok!(Currencies::deposit(erc20, &new_account, 100));

		// Assert - ERC20 uses sufficients
		let account_post = frame_system::Pallet::<Runtime>::account(&new_account);
		assert_eq!(account_post.providers, 0);
		assert_eq!(account_post.sufficients, 1); // Incremented
		assert_eq!(Currencies::free_balance(erc20, &new_account), 100);
	});
}

#[test]
fn deposit_should_not_increment_providers_on_existing_account() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let contract = deploy_token_contract();
		let erc20 = bind_erc20(contract);

		let new_account: AccountId = AccountId::from([37u8; 32]);
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			new_account.clone(),
			erc20,
			50,
		));
		let account_pre = frame_system::Pallet::<Runtime>::account(&new_account);
		assert_eq!(account_pre.providers, 0);
		assert_eq!(account_pre.sufficients, 1); // ERC20 uses sufficients

		// Withdraw some funds to holding account so deposit can be made
		assert_ok!(Currencies::withdraw(erc20, &ALICE.into(), 100));

		// Act
		assert_ok!(Currencies::deposit(erc20, &new_account, 100));

		// Assert that sufficients remained the same
		let account_post = frame_system::Pallet::<Runtime>::account(&new_account);
		assert_eq!(account_post.providers, 0);
		assert_eq!(account_post.sufficients, 1); // Still 1
		assert_eq!(Currencies::free_balance(erc20, &new_account), 150);
	});
}

#[test]
fn withdraw_should_decrement_providers_when_balance_becomes_zero() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let contract = deploy_token_contract();
		let erc20 = bind_erc20(contract);

		let new_account: AccountId = AccountId::from([37u8; 32]);
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(
			new_account.clone().into()
		)));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			new_account.clone(),
			erc20,
			100,
		));
		let account_pre = frame_system::Pallet::<Runtime>::account(&new_account);
		assert_eq!(account_pre.providers, 0);
		assert_eq!(account_pre.sufficients, 1); // ERC20 uses sufficients
		assert_eq!(Currencies::free_balance(erc20, &new_account), 100);

		// Act
		assert_ok!(Currencies::withdraw(erc20, &new_account, 100));

		// Assert - sufficients decremented
		let account_post = frame_system::Pallet::<Runtime>::account(&new_account);
		assert_eq!(account_post.providers, 0);
		assert_eq!(account_post.sufficients, 0); // Decremented
		assert_eq!(Currencies::free_balance(erc20, &new_account), 0);
	});
}

#[test]
fn withdraw_should_not_decrement_providers_when_partial_balance_remains() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let contract = deploy_token_contract();
		let erc20 = bind_erc20(contract);

		let new_account: AccountId = AccountId::from([37u8; 32]);
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			new_account.clone(),
			erc20,
			100,
		));

		let account_pre = frame_system::Pallet::<Runtime>::account(&new_account);
		assert_eq!(account_pre.providers, 0);
		assert_eq!(account_pre.sufficients, 1); // ERC20 uses sufficients

		// Act
		assert_ok!(Currencies::withdraw(erc20, &new_account, 50));

		// Assert that sufficients remained the same
		let account_post = frame_system::Pallet::<Runtime>::account(&new_account);
		assert_eq!(account_post.providers, 0);
		assert_eq!(account_post.sufficients, 1); // Still 1
		assert_eq!(Currencies::free_balance(erc20, &new_account), 50);
	});
}

#[test]
fn transfer_between_two_new_accounts_should_manage_providers_correctly() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let contract = deploy_token_contract();
		let erc20 = bind_erc20(contract);

		let sender_account: AccountId = AccountId::from([0x22; 32]);
		let receiver_account: AccountId = AccountId::from([0x33; 32]);

		// Give sender account balance
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			sender_account.clone(),
			erc20,
			100,
		));

		let sender_account_initial = frame_system::Pallet::<Runtime>::account(&sender_account);
		let receiver_account_initial = frame_system::Pallet::<Runtime>::account(&receiver_account);
		assert_eq!(sender_account_initial.providers, 0);
		assert_eq!(sender_account_initial.sufficients, 1); // ERC20 uses sufficients
		assert_eq!(receiver_account_initial.providers, 0);
		assert_eq!(receiver_account_initial.sufficients, 0);

		// Act - transfer all from sender to receiver (new account)
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(sender_account.clone()),
			receiver_account.clone(),
			erc20,
			100,
		));

		// Assert - sender sufficients decremented, receiver sufficients incremented
		let sender_account_final = frame_system::Pallet::<Runtime>::account(&sender_account);
		let receiver_account_final = frame_system::Pallet::<Runtime>::account(&receiver_account);
		assert_eq!(sender_account_final.providers, 0);
		assert_eq!(sender_account_final.sufficients, 0); // Decremented
		assert_eq!(receiver_account_final.providers, 0);
		assert_eq!(receiver_account_final.sufficients, 1); // Incremented
		assert_eq!(Currencies::free_balance(erc20, &sender_account), 0);
		assert_eq!(Currencies::free_balance(erc20, &receiver_account), 100);
	});
}

#[test]
fn erc20_transfer_works_with_providers_and_consumers() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// This test demonstrates that ERC20 (using sufficients) works alongside HDX (using providers)
		// and can handle consumers from reserves

		let account: AccountId = AccountId::from([37u8; 32]);
		let contract = deploy_token_contract();
		let erc20 = bind_erc20(contract);

		// STEP 1: Give account HDX (balances pallet increments providers)
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			account.clone(),
			HDX,
			500 * UNITS,
		));

		let state_after_hdx = frame_system::Pallet::<Runtime>::account(&account);
		assert_eq!(state_after_hdx.providers, 1); // HDX provider
		assert_eq!(state_after_hdx.consumers, 0);

		// STEP 2: Give account ERC20 tokens (now increments sufficients instead of providers)
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			account.clone(),
			erc20,
			100,
		));

		let state_after_erc20 = frame_system::Pallet::<Runtime>::account(&account);
		assert_eq!(state_after_erc20.providers, 1); // HDX only
		assert_eq!(state_after_erc20.sufficients, 1); // ERC20 uses sufficients
		assert_eq!(state_after_erc20.consumers, 0);

		// STEP 3: Reserve some HDX (this creates a consumer)
		// Reserved balance increments consumers but the account still has free balance
		let reserve_amount = 100 * UNITS;
		assert_ok!(hydradx_runtime::Balances::reserve_named(
			&[0u8; 8],
			&account,
			reserve_amount,
		));

		let state_with_reserve = frame_system::Pallet::<Runtime>::account(&account);
		assert_eq!(state_with_reserve.providers, 1); // HDX only
		assert_eq!(state_with_reserve.sufficients, 1); // ERC20 sufficients
		assert_eq!(state_with_reserve.consumers, 1); // Reserved balance created consumer
		assert_eq!(Currencies::free_balance(HDX, &account), 400 * UNITS); // 500 - 100 reserved
		assert_eq!(Currencies::free_balance(erc20, &account), 100);

		// STEP 4: Transfer away all free HDX (not the reserved amount)
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(account.clone()),
			ALICE.into(),
			HDX,
			400 * UNITS - UNITS, // All free balance
		));

		let state_after_hdx_transfer = frame_system::Pallet::<Runtime>::account(&account);
		assert_eq!(state_after_hdx_transfer.consumers, 1);
		assert_eq!(state_after_hdx_transfer.providers, 1); // HDX still has provider (reserved)
		assert_eq!(state_after_hdx_transfer.sufficients, 1); // ERC20 sufficients
		assert_eq!(Currencies::free_balance(HDX, &account), UNITS);
		assert_eq!(Currencies::free_balance(erc20, &account), 100);

		// STEP 5: Transfer ERC20 away
		// Should SUCCEED - sufficients can coexist with consumers/providers
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(account.clone()),
			ALICE.into(),
			erc20,
			100,
		));

		let final_state = frame_system::Pallet::<Runtime>::account(&account);
		assert_eq!(final_state.providers, 1); // Still have HDX provider (reserved)
		assert_eq!(final_state.consumers, 1);
		assert_eq!(final_state.sufficients, 0); // ERC20 sufficients decremented
		assert_eq!(Currencies::free_balance(erc20, &account), 0);

		//Step 6: Clean up by unreserving HDX
		hydradx_runtime::Balances::unreserve_named(&[0u8; 8], &account, reserve_amount);

		let final_state = frame_system::Pallet::<Runtime>::account(&account);
		assert_eq!(final_state.providers, 1); // HDX only (no erc20, no consumer)
		assert_eq!(final_state.consumers, 0);
		assert_eq!(Currencies::free_balance(erc20, &account), 0);
		assert_eq!(Currencies::free_balance(HDX, &account), 100 * UNITS + UNITS);

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(account.clone()),
			ALICE.into(),
			HDX,
			100 * UNITS + UNITS, // All free balance
		));

		let final_state = frame_system::Pallet::<Runtime>::account(&account);
		assert_eq!(final_state.providers, 0);
		assert_eq!(final_state.consumers, 0);
		assert_eq!(Currencies::free_balance(erc20, &account), 0);
	});
}

#[test]
fn nonce_should_become_u32_max_when_account_reaped_with_erc20_but_no_provider_management() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// This test previously demonstrated the bug where nonce became u32::MAX
		// Now with sufficients management, the bug is FIXED:
		// 1. Account has HDX (providers = 1)
		// 2. Account receives ERC20 tokens (sufficients = 1)
		// 3. Account transfers away all HDX (providers = 0, but sufficients = 1)
		// 4. Account is NOT reaped (sufficients keeps it alive), nonce stays 0

		let account: AccountId = AccountId::from([0xBB; 32]);
		let contract = deploy_token_contract();
		let erc20 = bind_erc20(contract);

		// STEP 1: Give account some HDX tokens (providers = 1)
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			account.clone(),
			HDX,
			100 * UNITS,
		));

		let state_after_hdx = frame_system::Pallet::<Runtime>::account(&account);
		assert_eq!(state_after_hdx.providers, 1);
		assert_eq!(state_after_hdx.sufficients, 0);
		assert_eq!(state_after_hdx.nonce, 0);

		// STEP 2: Give account ERC20 tokens
		// FIX: Now increments sufficients instead of providers
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			account.clone(),
			erc20,
			100,
		));

		let state_after_erc20 = frame_system::Pallet::<Runtime>::account(&account);
		assert_eq!(state_after_erc20.providers, 1); // HDX
		assert_eq!(state_after_erc20.sufficients, 1); // ERC20 uses sufficients
		assert_eq!(Currencies::free_balance(erc20, &account), 100);
		assert_eq!(state_after_erc20.nonce, 0);

		// STEP 3: Transfer away all HDX tokens
		// This decrements providers to 0, but sufficients is still 1
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(account.clone()),
			ALICE.into(),
			HDX,
			100 * UNITS,
		));

		// STEP 4: Check the state - account is NOT reaped
		let state_after_hdx_transfer = frame_system::Pallet::<Runtime>::account(&account);
		assert_eq!(
			state_after_hdx_transfer.providers, 0,
			"Providers = 0 (no HDX)"
		);
		assert_eq!(
			state_after_hdx_transfer.sufficients, 1,
			"Sufficients = 1 keeps account alive"
		);

		// BUG FIX: Nonce stays at 0 (account wasn't reaped)
		assert_eq!(
			state_after_hdx_transfer.nonce,
			0,
			"FIXED: Nonce stays 0 because account is not reaped (sufficients = 1)"
		);

		// Account still has ERC20 balance and is alive
		assert_eq!(
			Currencies::free_balance(erc20, &account),
			100,
			"ERC20 balance still exists and accessible"
		);
	});
}

use hydradx_runtime::MultiTransactionPayment;

#[test]
fn nonce_should_become_u32_max_for_native_evm_account_when_reaped() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// This test previously reproduced the bug for NATIVE EVM ACCOUNTS (truncated)
		// Now with sufficients management, the bug is FIXED:
		// 1. EVM truncated account has HDX (providers = 1)
		// 2. Account receives ERC20 tokens (sufficients = 1)
		// 3. Account transfers away all HDX (providers = 0, but sufficients = 1)
		// 4. Account is NOT reaped, nonce stays 0

		let contract = deploy_token_contract();
		let erc20 = bind_erc20(contract);

		// Create a native EVM account (truncated account - NOT bound)
		let evm_address = EvmAddress::from_low_u64_be(0xDEADBEEF);
		let truncated_account = EVMAccounts::truncated_account_id(evm_address);

		// Verify it's a truncated EVM account
		assert!(EVMAccounts::is_evm_account(truncated_account.clone()));

		// STEP 1: Give truncated account some HDX tokens (providers = 1)
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			truncated_account.clone(),
			HDX,
			100 * UNITS,
		));

		let state_after_hdx = frame_system::Pallet::<Runtime>::account(&truncated_account);
		assert_eq!(state_after_hdx.providers, 1);
		assert_eq!(state_after_hdx.sufficients, 0);
		assert_eq!(state_after_hdx.nonce, 0);

		// STEP 2: Give truncated account ERC20 tokens
		// FIX: Now increments sufficients instead of providers
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			truncated_account.clone(),
			erc20,
			100,
		));

		let state_after_erc20 = frame_system::Pallet::<Runtime>::account(&truncated_account);
		assert_eq!(state_after_erc20.providers, 1); // HDX
		assert_eq!(state_after_erc20.sufficients, 1); // ERC20 uses sufficients
		assert_eq!(Currencies::free_balance(erc20, &truncated_account), 100);
		assert_eq!(state_after_erc20.nonce, 0);

		assert_ok!(MultiTransactionPayment::add_currency(
			RuntimeOrigin::root(),
			erc20,
			FixedU128::from_rational(1, 4)
		));

		assert_ok!(MultiTransactionPayment::set_currency(
			RuntimeOrigin::signed(truncated_account.clone()),
			erc20,
		));

		// STEP 3: Transfer away all HDX tokens
		// This decrements providers to 0, but sufficients is still 1
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(truncated_account.clone()),
			ALICE.into(),
			HDX,
			100 * UNITS,
		));

		// STEP 4: Check the state - account is NOT reaped
		let state_after_hdx_transfer = frame_system::Pallet::<Runtime>::account(&truncated_account);
		assert_eq!(
			state_after_hdx_transfer.providers, 0,
			"Providers = 0 (no HDX)"
		);
		assert_eq!(
			state_after_hdx_transfer.sufficients, 1,
			"Sufficients = 1 keeps account alive"
		);

		// BUG FIX: Nonce stays at 0 for EVM accounts (not reaped)
		assert_eq!(
			state_after_hdx_transfer.nonce,
			0,
			"FIXED: Nonce stays 0 because account is not reaped (sufficients = 1)"
		);

		// Account still has ERC20 balance and is accessible
		assert_eq!(
			Currencies::free_balance(erc20, &truncated_account),
			100,
			"ERC20 balance still exists and accessible"
		);
	});
}
