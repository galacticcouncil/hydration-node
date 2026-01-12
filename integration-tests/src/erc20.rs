use crate::evm::MockHandle;
use crate::polkadot_test_net::*;
use crate::utils::contracts::*;
use ethabi::ethereum_types::BigEndianHash;
use fp_evm::ExitReason::Succeed;
use fp_evm::PrecompileSet;
use frame_support::pallet_prelude::DispatchError;
use frame_support::pallet_prelude::DispatchError::Other;
use frame_support::storage::with_transaction;
use frame_support::traits::ExistenceRequirement;
use frame_support::{assert_noop, assert_ok};
use hex_literal::hex;
use sp_runtime::traits::Convert;

use hydradx_runtime::evm::evm_error_decoder::EvmErrorDecoder;
use hydradx_runtime::evm::precompiles::HydraDXPrecompiles;
use hydradx_runtime::evm::{Erc20Currency, EvmNonceProvider as AccountNonce, Executor, Function};
use hydradx_runtime::AssetRegistry;
use hydradx_runtime::RuntimeCall;
use hydradx_runtime::RuntimeOrigin;
use hydradx_runtime::{AssetLocation, Currencies};
use hydradx_runtime::{EVMAccounts, Runtime};
use hydradx_traits::evm::CallContext;
use hydradx_traits::evm::ERC20;
use hydradx_traits::evm::EVM;
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use orml_traits::MultiCurrency;
use pallet_evm::ExitSucceed::Returned;
use primitives::EvmAddress;
use sp_core::bounded_vec::BoundedVec;

use pallet_evm_accounts::EvmNonceProvider;
use polkadot_xcm::v5::Junction::AccountKey20;
use polkadot_xcm::v5::Location;
use primitives::AccountId;
use sp_core::Encode;
use sp_core::{H256, U256};
use sp_runtime::{Permill, TransactionOutcome};
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
			Some(AssetLocation(Location::new(
				0,
				[AccountKey20 {
					key: contract.into(),
					network: None,
				}],
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
		let call_result = Executor::<Runtime>::view(context, data, 100_000);

		assert_eq!(call_result.exit_reason, Succeed(Returned));
		assert_ne!(call_result.value, vec![0; call_result.value.len()]);
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
		let call_result = Executor::<Runtime>::call(context, data, U256::zero(), 100_000);

		assert_eq!(call_result.exit_reason, Succeed(Returned));
		assert_ne!(call_result.value, vec![0; call_result.value.len()]);

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
			100,
			ExistenceRequirement::AllowDeath
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

		assert_ok!(Currencies::withdraw(
			asset,
			&ALICE.into(),
			100,
			ExistenceRequirement::AllowDeath
		));
	});
}

#[test]
fn deposit() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let contract = deploy_token_contract();
		let asset = bind_erc20(contract);
		assert_ok!(Currencies::withdraw(
			asset,
			&ALICE.into(),
			100,
			ExistenceRequirement::AllowDeath
		));

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

mod error_handling {
	use super::*;
	use frame_support::assert_noop;
	use hydradx_runtime::evm::Executor;
	use hydradx_runtime::Runtime;
	use hydradx_traits::evm::CallContext;
	use primitives::EvmAddress;
	use sp_core::keccak_256;

	#[test]
	fn out_of_gas_error_should_be_mapped() {
		TestNet::reset();

		Hydra::execute_with(|| {
			// Deploy GasEatingToken which consumes 380k gas in transfer
			let contract = crate::utils::contracts::deploy_contract("GasEater", crate::contracts::deployer());
			let asset = crate::erc20::bind_erc20(contract);

			// Try to transfer with insufficient gas
			assert_noop!(
				Currencies::transfer(RuntimeOrigin::signed(ALICE.into()), BOB.into(), asset, 100,),
				pallet_dispatcher::Error::<Runtime>::EvmOutOfGas
			);
		});
	}

	#[test]
	fn legacy_erc20_transfer_error_not_mapped_as_we_dont_have_such_contract_on_chain() {
		TestNet::reset();
		Hydra::execute_with(|| {
			let token = deploy_contract("LegacyERC20", crate::erc20::deployer());

			let asset = crate::erc20::bind_erc20(token);

			assert_noop!(
				Currencies::transfer(
					RuntimeOrigin::signed(ALICE.into()),
					BOB.into(),
					asset,
					100000000000000000000000 * UNITS
				),
				Other("evm:0x08c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000002645524332303a207472616e7366657220616d6f756e7420657863656564732062616c616e63650000000000000000000000000000000000000000000000000000")
			);
		});
	}

	#[test]
	fn new_solidity_named_error_selectors_are_not_mapped_as_we_dont_have_such_contracts() {
		TestNet::reset();
		Hydra::execute_with(|| {
			let contract = crate::erc20::deploy_token_contract();
			let asset = crate::erc20::bind_erc20(contract);
			assert_noop!(
				Currencies::transfer(RuntimeOrigin::signed(BOB.into()), ALICE.into(), asset, 100),
				DispatchError::Other("evm:0xe450d38c000000000000000000000000050505050505050505050505050505050505050500000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064")
			);
		});
	}

	#[test]
	fn arithmetic_underflow_should_be_decoded() {
		TestNet::reset();
		Hydra::execute_with(|| {
			// Deploy WeirdToken which has causeUnderflow function
			let contract = deploy_contract("WeirdToken", crate::erc20::deployer());

			// Call causeUnderflow(1) which will trigger Panic(0x11)
			let mut data = vec![0u8; 4];
			// Function selector for causeUnderflow(uint256)
			let selector = keccak_256(b"causeUnderflow(uint256)");
			data[0..4].copy_from_slice(&selector[0..4]);
			// Append uint256 parameter (1)
			data.extend_from_slice(&[0u8; 31]);
			data.push(1);

			let context = CallContext {
				contract,
				sender: crate::erc20::deployer(),
				origin: crate::erc20::deployer(),
			};

			let call_result = Executor::<Runtime>::view(context, data, 100_000);

			// Should get a Revert with Panic(0x11) data
			assert!(matches!(call_result.exit_reason, fp_evm::ExitReason::Revert(_)));

			assert_eq!(
				EvmErrorDecoder::convert(call_result),
				pallet_dispatcher::Error::<Runtime>::EvmArithmeticOverflowOrUnderflow.into()
			);
		});
	}

	#[test]
	fn arithmetic_overflow_should_be_decoded() {
		TestNet::reset();
		Hydra::execute_with(|| {
			// Deploy WeirdToken which has causeOverflow function
			let contract = deploy_contract("WeirdToken", crate::erc20::deployer());

			// Call causeOverflow() which will trigger Panic(0x11)
			let selector = keccak_256(b"causeOverflow()");
			let data = selector[0..4].to_vec();

			let context = CallContext {
				contract,
				sender: crate::erc20::deployer(),
				origin: crate::erc20::deployer(),
			};

			let call_result = Executor::<Runtime>::view(context, data, 100_000);

			// Should get a Revert with Panic(0x11) data
			assert!(matches!(call_result.exit_reason, fp_evm::ExitReason::Revert(_)));

			assert_eq!(
				EvmErrorDecoder::convert(call_result),
				pallet_dispatcher::Error::<Runtime>::EvmArithmeticOverflowOrUnderflow.into()
			);
		});
	}

	#[test]
	fn transfer_dispatch_error_can_be_decoded_when_insufficient_balance() {
		TestNet::reset();
		Hydra::execute_with(|| {
			// Deploy HydraToken
			let _contract = deploy_contract("HydraToken", crate::erc20::deployer());

			let contract: EvmAddress = hex!["0000000000000000000000000000000100000005"].into();

			// Try to transfer more tokens than BOB has (BOB has 0 balance)
			// Prepare transfer(address,uint256) call data
			let selector = keccak_256(b"transfer(address,uint256)");
			let mut data = selector[0..4].to_vec();
			// Recipient address
			data.extend_from_slice(H256::from(EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE))).as_bytes());
			// Amount to transfer)
			data.extend_from_slice(H256::from_uint(&U256::from(1_000_000_000 * 10u128.pow(18))).as_bytes());

			//Act
			let context = CallContext {
				contract,
				sender: EVMAccounts::evm_address(&Into::<AccountId>::into(BOB)), // BOB has no tokens
				origin: EVMAccounts::evm_address(&Into::<AccountId>::into(BOB)),
			};

			let call_result = Executor::<Runtime>::call(context, data, U256::zero(), 100_000);

			assert!(matches!(call_result.exit_reason, fp_evm::ExitReason::Revert(_)));
			assert_eq!(
				EvmErrorDecoder::convert(call_result),
				orml_tokens::Error::<Runtime>::BalanceTooLow.into()
			);
		});
	}
}
