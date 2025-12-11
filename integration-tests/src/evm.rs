#![cfg(test)]

use crate::{assert_balance, polkadot_test_net::*};
use codec::Decode;

use fp_evm::{Context, Transfer};
use fp_rpc::runtime_decl_for_ethereum_runtime_rpc_api::EthereumRuntimeRPCApi;
use frame_support::dispatch::DispatchInfo;
use frame_support::pallet_prelude::InvalidTransaction;
use frame_support::storage::with_transaction;
use frame_support::traits::fungible::Mutate;
use frame_support::unsigned::TransactionValidityError;
use frame_support::weights::Weight;
use hydradx_runtime::evm::precompiles::CALLPERMIT;

use crate::utils::accounts::alith_evm_account;
use crate::utils::accounts::alith_evm_address;
use crate::utils::accounts::alith_secret_key;
use crate::utils::accounts::alith_truncated_account;
use crate::utils::accounts::MockAccount;
use frame_support::{assert_ok, dispatch::GetDispatchInfo, sp_runtime::codec::Encode, traits::Contains};
use frame_system::RawOrigin;
use hex_literal::hex;
use hydradx_runtime::evm::Erc20Currency;
use hydradx_runtime::MultiTransactionPayment;
use hydradx_runtime::Runtime;
use hydradx_traits::evm::CallContext;
use hydradx_traits::evm::ERC20;

use libsecp256k1::sign;
use libsecp256k1::Message;

use libsecp256k1::SecretKey;
use sp_core::bounded_vec::BoundedVec;

use hydradx_runtime::evm::precompiles::DISPATCH_ADDR;
use hydradx_runtime::evm::EvmAddress;
use hydradx_runtime::evm::ExtendedAddressMapping;
use hydradx_runtime::evm::Function;
use hydradx_runtime::XYK;
use hydradx_runtime::{
	evm::precompiles::{
		handle::EvmDataWriter, multicurrency::MultiCurrencyPrecompile, Address, Bytes, HydraDXPrecompiles,
	},
	AssetRegistry, Balances, CallFilter, Currencies, EVMAccounts, Omnipool, RuntimeCall, RuntimeOrigin, System, Tokens,
	TransactionPause, EVM,
};
use hydradx_traits::router::{PoolType, Trade};
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use orml_traits::MultiCurrency;
use pallet_evm::*;
use pretty_assertions::assert_eq;
use primitives::{AssetId, Balance};
use sp_core::{blake2_256, H160, H256, U256};
use sp_runtime::traits::Dispatchable;
use sp_runtime::TransactionOutcome;
use sp_runtime::{traits::SignedExtension, DispatchError, FixedU128, Permill};
use std::{borrow::Cow, cmp::Ordering};
use xcm_emulator::TestExt;

pub const TREASURY_ACCOUNT_INIT_BALANCE: Balance = 1000 * UNITS;

mod account_conversion {
	use super::*;
	use crate::erc20::{bind_erc20, deploy_token_contract, deployer};
	use fp_evm::ExitSucceed;
	use frame_support::{assert_noop, assert_ok};
	use hydradx_runtime::evm::Erc20Currency;
	use hydradx_runtime::Runtime;
	use hydradx_traits::evm::CallContext;
	use pretty_assertions::assert_eq;
	use sp_core::Pair;
	use sp_runtime::traits::IdentifyAccount;

	#[test]
	fn eth_address_should_convert_to_truncated_address_when_not_bound() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE));
			// truncated address
			let substrate_address: AccountId = EVMAccounts::truncated_account_id(evm_address);

			assert_eq!(ExtendedAddressMapping::into_account_id(evm_address), substrate_address);

			assert_eq!(EVMAccounts::account_id(evm_address), substrate_address);
			assert_eq!(EVMAccounts::bound_account_id(evm_address), None);
		});
	}

	#[test]
	fn eth_address_should_convert_to_full_address_when_bound() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let substrate_address: AccountId = Into::<AccountId>::into(ALICE);
			let evm_address = EVMAccounts::evm_address(&substrate_address);

			assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(
				substrate_address.clone()
			)));

			assert_eq!(ExtendedAddressMapping::into_account_id(evm_address), substrate_address);

			assert_eq!(EVMAccounts::account_id(evm_address), substrate_address);
			assert_eq!(EVMAccounts::bound_account_id(evm_address), Some(substrate_address));
		});
	}

	#[test]
	fn bind_address_should_fail_when_already_bound() {
		TestNet::reset();

		Hydra::execute_with(|| {
			assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())),);

			assert_noop!(
				EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())),
				pallet_evm_accounts::Error::<Runtime>::AddressAlreadyBound,
			);
		});
	}

	#[test]
	fn bind_address_should_fail_when_nonce_is_not_zero() {
		use pallet_evm_accounts::EvmNonceProvider;
		TestNet::reset();

		Hydra::execute_with(|| {
			// Arrange
			let evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE));
			let truncated_address = EVMAccounts::truncated_account_id(evm_address);

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				truncated_address,
				WETH,
				100 * UNITS as i128,
			));

			let data =
				hex!["4d0045544800d1820d45118d78d091e685490c674d7596e62d1f0000000000000000140000000f0000c16ff28623"]
					.to_vec();

			// Act
			assert_ok!(EVM::call(
				evm_signed_origin(evm_address),
				evm_address,
				DISPATCH_ADDR,
				data,
				U256::from(0),
				1000000,
				gas_price(),
				None,
				Some(U256::zero()),
				[].into()
			));

			// Assert
			assert!(hydradx_runtime::evm::EvmNonceProvider::get_nonce(evm_address) != U256::zero());

			assert_noop!(
				EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())),
				pallet_evm_accounts::Error::<Runtime>::TruncatedAccountAlreadyUsed,
			);
		});
	}

	#[test]
	fn truncated_address_should_be_used_in_evm_precompile_when_not_bound() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE));
			let truncated_address = EVMAccounts::truncated_account_id(evm_address);

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				truncated_address,
				HDX,
				100 * UNITS as i128,
			));

			let data = EvmDataWriter::new_with_selector(Function::BalanceOf)
				.write(Address::from(evm_address))
				.build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address,
					caller: evm_address,
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: true,
			};

			//Act
			let result = MultiCurrencyPrecompile::<Runtime>::execute(&mut handle);

			//Assert

			// 100 * UNITS, balance of truncated_address
			let expected_output = hex! {"
				00000000000000000000000000000000 000000000000000000005AF3107A4000
			"};

			assert_eq!(
				result,
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: expected_output.to_vec()
				})
			);
		});
	}

	#[test]
	fn full_address_should_be_used_in_evm_precompile_when_bound() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE));

			let data = EvmDataWriter::new_with_selector(Function::BalanceOf)
				.write(Address::from(evm_address))
				.build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address,
					caller: evm_address,
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: true,
			};

			//Act
			assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())),);

			let result = MultiCurrencyPrecompile::<Runtime>::execute(&mut handle);

			//Assert

			// 1000 * UNITS, balance of ALICE
			let expected_output = hex! {"
				00000000000000000000000000000000 000000000000000000038D7EA4C68000
			"};
			assert_eq!(
				result,
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: expected_output.to_vec()
				})
			);
		});
	}

	#[test]
	fn bind_evm_address_tx_cost_should_be_increased_by_fee_multiplier() {
		// the fee multiplier is in the pallet evm accounts config and the desired fee is 10 HDX
		use pallet_transaction_payment::{Multiplier, NextFeeMultiplier};
		use primitives::constants::currency::UNITS;
		use sp_runtime::FixedPointNumber;

		TestNet::reset();

		Hydra::execute_with(|| {
			let call = pallet_evm_accounts::Call::<Runtime>::bind_evm_address {};
			let info = call.get_dispatch_info();
			// convert to outer call
			let call = RuntimeCall::EVMAccounts(call);
			let len = call.using_encoded(|e| e.len()) as u32;

			NextFeeMultiplier::<Runtime>::put(Multiplier::saturating_from_integer(1));
			let fee_raw = hydradx_runtime::TransactionPayment::compute_fee_details(len, &info, 0);
			let fee = fee_raw.final_fee();

			// assert that the fee is within some range
			assert!(fee > 2 * UNITS);
			assert!(fee < 4 * UNITS);
		});
	}

	#[test]
	fn evm_call_from_runtime_rpc_should_be_accepted_from_unbound_addresses() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let data =
				hex!["4d0045544800d1820d45118d78d091e685490c674d7596e62d1f0000000000000000140000000f0000c16ff28623"]
					.to_vec();

			//Act & Assert
			assert_ok!(hydradx_runtime::Runtime::call(
				evm_address(), // from
				DISPATCH_ADDR, // to
				data,          // data
				U256::from(1000u64),
				U256::from(100000u64),
				None,
				None,
				None,
				false,
				None,
			));
		});
	}

	#[test]
	fn evm_transaction_with_low_weight_should_work_having_no_out_of_gas_error() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			Balances::set_balance(&evm_account(), 1000 * UNITS);

			let data =
				hex!["4f003679d1d8e31d312a55f7ca994773b6a4fc7a92f07d898ae86bad4f3cab303c49000000000b00a0724e1809"]
					.to_vec();

			//Act & Assert
			let res = hydradx_runtime::Runtime::call(
				evm_address(), // from
				DISPATCH_ADDR, // to
				data,          // data
				U256::from(0u64),
				U256::from(60000u64),
				None,
				None,
				None,
				false,
				None,
			);

			assert_eq!(
				res.clone().unwrap().exit_reason,
				ExitReason::Succeed(ExitSucceed::Stopped)
			);

			println!("{:?}", res);
		});
	}

	#[test]
	fn evm_call_from_runtime_rpc_should_not_be_accepted_from_bound_addresses() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let data =
				hex!["4d0045544800d1820d45118d78d091e685490c674d7596e62d1f0000000000000000140000000f0000c16ff28623"]
					.to_vec();

			assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())),);

			let evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE));

			//Act & Assert
			assert_noop!(
				hydradx_runtime::Runtime::call(
					evm_address,   // from
					DISPATCH_ADDR, // to
					data,          // data
					U256::from(1000u64),
					U256::from(100000u64),
					None,
					None,
					None,
					false,
					None,
				),
				pallet_evm_accounts::Error::<Runtime>::BoundAddressCannotBeUsed
			);
		});
	}

	#[test]
	fn estimation_of_evm_call_should_be_accepted_even_from_bound_address() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let data =
				hex!["4d0045544800d1820d45118d78d091e685490c674d7596e62d1f0000000000000000140000000f0000c16ff28623"]
					.to_vec();

			assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())),);

			let evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE));

			//Act & Assert
			assert_ok!(hydradx_runtime::Runtime::call(
				evm_address,   // from
				DISPATCH_ADDR, // to
				data,          // data
				U256::from(1000u64),
				U256::from(100000u64),
				None,
				None,
				None,
				true,
				None,
			));
		});
	}

	#[test]
	fn claim_account_should_work_for_account_with_erc20_balance() {
		TestNet::reset();
		Hydra::execute_with(|| {
			// Arrange
			let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
			let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();
			let evm_address = EVMAccounts::evm_address(&account);

			let contract = deploy_token_contract();
			let asset = bind_erc20(contract);

			assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
				RuntimeOrigin::root(),
				asset,
				FixedU128::from_rational(1, 10),
			));

			assert_ok!(<Erc20Currency<Runtime> as ERC20>::transfer(
				CallContext {
					contract,
					sender: deployer(),
					origin: deployer()
				},
				evm_address,
				1_000_000_000_000_000_000
			));

			std::assert_eq!(
				Erc20Currency::<Runtime>::balance_of(CallContext::new_view(contract), evm_address),
				1_000_000_000_000_000_000
			);

			assert!(!System::account_exists(&account));
			assert_eq!(System::account_nonce(&account), 0);
			assert_eq!(System::sufficients(&account), 0);

			let signature = pallet_evm_accounts::sign_message::<Runtime>(pair, &account, asset);

			// Act
			assert_ok!(EVMAccounts::claim_account(
				RuntimeOrigin::none(),
				account.clone(),
				asset,
				signature
			),);

			// Assert
			assert_eq!(
				hydradx_runtime::MultiTransactionPayment::account_currency(&account),
				asset
			);

			assert_eq!(System::account_nonce(&account), 0);
			assert_eq!(System::sufficients(&account), 1);

			let evm_address = EVMAccounts::evm_address(&account);

			std::assert_eq!(EVMAccounts::bound_account_id(evm_address), Some(account.clone()));

			std::assert_eq!(EVMAccounts::account_id(evm_address), account);
		});
	}

	#[test]
	fn claim_account_should_enable_submitting_substrate_transactions_from_claimed_account() {
		TestNet::reset();
		Hydra::execute_with(|| {
			// Arrange
			let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
			let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();
			let evm_address = EVMAccounts::evm_address(&account);

			let contract = deploy_token_contract();
			let asset = bind_erc20(contract);

			assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
				RuntimeOrigin::root(),
				asset,
				FixedU128::from_rational(1, 10),
			));

			assert_ok!(<Erc20Currency<Runtime> as ERC20>::transfer(
				CallContext {
					contract,
					sender: deployer(),
					origin: deployer()
				},
				evm_address,
				1_000_000_000_000_000_000
			));

			let call = RuntimeCall::MultiTransactionPayment(pallet_transaction_multi_payment::Call::set_currency {
				currency: asset,
			});

			let info = DispatchInfo {
				weight: Weight::from_parts(106_957_000, 0),
				..Default::default()
			};
			let len: usize = 10;

			let nonce = System::account_nonce(&account);
			let check_nonce_pre =
				frame_system::CheckNonce::<Runtime>::from(nonce).pre_dispatch(&account, &call, &info, len);
			assert_noop!(
				check_nonce_pre,
				TransactionValidityError::Invalid(InvalidTransaction::Payment)
			);

			let signature = pallet_evm_accounts::sign_message::<Runtime>(pair, &account, asset);

			// Act
			assert_ok!(EVMAccounts::claim_account(
				RuntimeOrigin::none(),
				account.clone(),
				asset,
				signature
			),);

			// Assert
			let nonce = System::account_nonce(&account);
			let check_nonce_pre =
				frame_system::CheckNonce::<Runtime>::from(nonce).pre_dispatch(&account, &call, &info, len);

			let pre = pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(0)
				.pre_dispatch(&account, &call, &info, len);
			assert_ok!(&pre);

			let result = call.clone().dispatch(RuntimeOrigin::signed(account.clone().into()));
			assert_ok!(result);

			assert_ok!(
				pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::post_dispatch(
					Some(pre.unwrap()),
					&info,
					&frame_support::dispatch::PostDispatchInfo::default(),
					len,
					&Ok(())
				)
			);
			assert_ok!(check_nonce_pre);
		});
	}

	#[test]
	fn claim_account_should_not_be_deleted_when_all_assets_removed() {
		TestNet::reset();
		Hydra::execute_with(|| {
			// Arrange
			let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
			let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();
			let evm_address = EVMAccounts::evm_address(&account);

			let contract = deploy_token_contract();
			let asset = bind_erc20(contract);

			assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
				RuntimeOrigin::root(),
				asset,
				FixedU128::from_rational(1, 10),
			));

			assert_ok!(<Erc20Currency<Runtime> as ERC20>::transfer(
				CallContext {
					contract,
					sender: deployer(),
					origin: deployer()
				},
				evm_address,
				1_000_000_000_000_000_000
			));

			assert!(!System::account_exists(&account));
			assert_eq!(System::account_nonce(&account), 0);
			assert_eq!(System::sufficients(&account), 0);

			let signature = pallet_evm_accounts::sign_message::<Runtime>(pair, &account, asset);
			assert_ok!(EVMAccounts::claim_account(
				RuntimeOrigin::none(),
				account.clone(),
				asset,
				signature
			),);

			// Act
			assert_ok!(<Erc20Currency<Runtime> as MultiCurrency<AccountId>>::transfer(
				contract,
				&account,
				&AccountId::from(BOB),
				1_000_000_000_000_000_000
			));

			std::assert_eq!(Erc20Currency::<Runtime>::free_balance(contract, &account), 0);

			// Assert
			assert!(System::account_exists(&account));
			assert_eq!(System::account_nonce(&account), 0);
			assert_eq!(System::sufficients(&account), 1);
		});
	}
}

mod standard_precompiles {
	use super::*;
	use frame_support::assert_ok;
	use pretty_assertions::assert_eq;
	use sp_runtime::traits::UniqueSaturatedInto;

	fn evm_runner_call(
		to: EvmAddress,
		data: Vec<u8>,
	) -> Result<CallInfo, RunnerError<Error<hydradx_runtime::Runtime>>> {
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			evm_account(),
			WETH,
			to_ether(1_000),
			0,
		));
		<hydradx_runtime::Runtime as Config>::Runner::call(
			evm_address(),
			to,
			data,
			U256::from(1000u64),
			U256::from(1000000u64).unique_saturated_into(),
			None,
			None,
			None,
			Default::default(),
			false,
			true,
			None,
			None,
			<hydradx_runtime::Runtime as Config>::config(),
		)
	}

	#[test]
	fn ecrecover_precompile() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let input = hex! {"
			18c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c
			000000000000000000000000000000000000000000000000000000000000001c
			73b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75f
			eeb940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549
		"}
			.to_vec();
			let expected_output = hex!("000000000000000000000000a94f5374fce5edbc8e2a8697c15331677e6ebf0b").to_vec();

			//Act
			let execution_result = evm_runner_call(hydradx_runtime::evm::precompiles::ECRECOVER, input).unwrap();

			//Assert
			assert_eq!(execution_result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned),);
			assert_eq!(execution_result.value, expected_output);
		});
	}

	#[test]
	fn sha256_precompile() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let input = "HydraDX".as_bytes().to_vec();
			let expected_output = hex!("61e6380e10376b3479838d623b2b1faeaa2afafcfaff2840a6df2f41161488da").to_vec();

			//Act
			let execution_result = evm_runner_call(hydradx_runtime::evm::precompiles::SHA256, input).unwrap();

			//Assert
			assert_eq!(execution_result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned),);
			assert_eq!(execution_result.value, expected_output);
		});
	}

	#[test]
	fn ripemd160_precompile() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let input = "HydraDX".as_bytes().to_vec();
			let mut expected_output = [0u8; 32];
			expected_output[12..32].copy_from_slice(&hex!("8883ba5c203439408542b87526c113426ce94742"));

			//Act
			let execution_result = evm_runner_call(hydradx_runtime::evm::precompiles::RIPEMD, input).unwrap();

			//Assert
			assert_eq!(execution_result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned),);
			assert_eq!(execution_result.value, expected_output);
		});
	}

	#[test]
	fn identity_precompile() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let input = "HydraDX".as_bytes().to_vec();

			//Act
			let execution_result = evm_runner_call(hydradx_runtime::evm::precompiles::IDENTITY, input.clone()).unwrap();

			//Assert
			assert_eq!(execution_result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned),);
			assert_eq!(execution_result.value, input);
		});
	}

	#[test]
	fn modexp_precompile() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let input = hex!(
				"
				0000000000000000000000000000000000000000000000000000000000000001
				0000000000000000000000000000000000000000000000000000000000000001
				0000000000000000000000000000000000000000000000000000000000000001
				03
				05
				07
				"
			)
			.to_vec();
			let expected_output = vec![5];

			//Act
			let execution_result = evm_runner_call(hydradx_runtime::evm::precompiles::MODEXP, input).unwrap();

			//Assert
			assert_eq!(execution_result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned),);
			assert_eq!(execution_result.value, expected_output);
		});
	}

	#[test]
	fn bn128add_precompile() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let input = hex!("089142debb13c461f61523586a60732d8b69c5b38a3380a74da7b2961d867dbf2d5fc7bbc013c16d7945f190b232eacc25da675c0eb093fe6b9f1b4b4e107b3625f8c89ea3437f44f8fc8b6bfbb6312074dc6f983809a5e809ff4e1d076dd5850b38c7ced6e4daef9c4347f370d6d8b58f4b1d8dc61a3c59d651a0644a2a27cf").to_vec();
			let expected_output = hex!("0a6678fd675aa4d8f0d03a1feb921a27f38ebdcb860cc083653519655acd6d79172fd5b3b2bfdd44e43bcec3eace9347608f9f0a16f1e184cb3f52e6f259cbeb").to_vec();

			//Act
			let execution_result = evm_runner_call(hydradx_runtime::evm::precompiles::BN_ADD, input).unwrap();

			//Assert
			assert_eq!(execution_result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned),);
			assert_eq!(execution_result.value, expected_output);
		});
	}

	#[test]
	fn bn128mul_precompile() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let input = hex!("089142debb13c461f61523586a60732d8b69c5b38a3380a74da7b2961d867dbf2d5fc7bbc013c16d7945f190b232eacc25da675c0eb093fe6b9f1b4b4e107b36ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff").to_vec();
			let expected_output = hex!("0bf982b98a2757878c051bfe7eee228b12bc69274b918f08d9fcb21e9184ddc10b17c77cbf3c19d5d27e18cbd4a8c336afb488d0e92c18d56e64dd4ea5c437e6").to_vec();

			//Act
			let execution_result = evm_runner_call(hydradx_runtime::evm::precompiles::BN_MUL, input).unwrap();

			//Assert
			assert_eq!(execution_result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned),);
			assert_eq!(execution_result.value, expected_output);
		});
	}

	#[test]
	fn bn128pairing_precompile() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let input = hex!("089142debb13c461f61523586a60732d8b69c5b38a3380a74da7b2961d867dbf2d5fc7bbc013c16d7945f190b232eacc25da675c0eb093fe6b9f1b4b4e107b3629f2c1dbcc614745f242077001ec9edd475acdab9ab435770d456bd22bbd2abf268683f9b1be0bde4508e2e25e51f6b44da3546e87524337d506fd03c4ff7ce01851abe58ef4e08916bec8034ca62c04cd08340ab6cc525e61706340926221651b71422869c92e49465200ca19033a8aa425f955be3d8329c4475503e45c00e1").to_vec();
			let expected_output = hex!("0000000000000000000000000000000000000000000000000000000000000000").to_vec();

			//Act
			let execution_result = evm_runner_call(hydradx_runtime::evm::precompiles::BN_PAIRING, input).unwrap();

			//Assert
			assert_eq!(execution_result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned),);
			assert_eq!(execution_result.value, expected_output);
		});
	}

	#[test]
	fn blake2f_precompile() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let input = hex!("0000000c48c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b6bbd41fbabd9831f79217e1319cde05b61626300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000001").to_vec();
			let expected_output = hex!("ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d17d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923").to_vec();

			//Act
			let execution_result = evm_runner_call(hydradx_runtime::evm::precompiles::BLAKE2F, input).unwrap();

			//Assert
			assert_eq!(execution_result.exit_reason, ExitReason::Succeed(ExitSucceed::Returned),);
			assert_eq!(execution_result.value, expected_output);
		});
	}
}

mod currency_precompile {
	use super::*;
	use fp_evm::ExitRevert::Reverted;
	use fp_evm::PrecompileFailure;
	use frame_support::assert_noop;
	use pretty_assertions::assert_eq;

	type AllHydraDXPrecompile = HydraDXPrecompiles<hydradx_runtime::Runtime>;
	type CurrencyPrecompile = MultiCurrencyPrecompile<hydradx_runtime::Runtime>;

	#[test]
	fn all_hydra_precompile_should_match_native_asset_address() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let data = EvmDataWriter::new_with_selector(Function::Name).build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: native_asset_ethereum_address(),
					caller: evm_address(),
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: true,
			};

			//Act
			let result = AllHydraDXPrecompile::new().execute(&mut handle);

			//Assert
			assert!(result.is_some());
			let output = EvmDataWriter::new().write(Bytes::from("HDX".as_bytes())).build();

			assert_eq!(
				result,
				Some(Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output
				}))
			);
		});
	}

	#[test]
	fn all_hydra_precompile_should_match_asset_address_with_max_asset_value() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let data = EvmDataWriter::new_with_selector(Function::Name).build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: H160::from(hex!("00000000000000000000000000000001ffffffff")),
					caller: evm_address(),
					apparent_value: U256::from(0),
				},
				code_address: H160::from(hex!("00000000000000000000000000000001ffffffff")),
				is_static: true,
			};

			//Act
			let result = AllHydraDXPrecompile::new().execute(&mut handle);

			//Assert
			assert!(result.is_some());
			assert_eq!(
				result,
				Some(Err(PrecompileFailure::Error {
					exit_status: ExitError::Other("Non-existing asset.".into()),
				}))
			);
		});
	}

	#[test]
	fn precompile_for_currency_name_should_work() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let data = EvmDataWriter::new_with_selector(Function::Name).build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: native_asset_ethereum_address(),
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: true,
			};

			//Act
			let result = CurrencyPrecompile::execute(&mut handle);

			//Assert
			let output = EvmDataWriter::new().write(Bytes::from("HDX".as_bytes())).build();
			assert_eq!(
				result,
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output
				})
			);
		});
	}

	#[test]
	fn precompile_for_currency_symbol_should_work() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			AssetRegistry::update(
				RuntimeOrigin::root(),
				HDX,
				Some(b"xHDX".to_vec().try_into().unwrap()),
				None,
				None,
				None,
				None,
				Some(b"xHDX".to_vec().try_into().unwrap()),
				Some(12u8),
				None,
			)
			.unwrap();

			let data = EvmDataWriter::new_with_selector(Function::Symbol).build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: native_asset_ethereum_address(),
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: true,
			};

			//Act
			let result = CurrencyPrecompile::execute(&mut handle);

			//Assert
			let output = EvmDataWriter::new().write(Bytes::from("xHDX".as_bytes())).build();
			assert_eq!(
				result,
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output
				})
			);
		});
	}

	#[test]
	fn precompile_for_currency_decimal_should_work() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			AssetRegistry::update(
				RuntimeOrigin::root(),
				HDX,
				Some(b"xHDX".to_vec().try_into().unwrap()),
				None,
				None,
				None,
				None,
				None,
				Some(12u8),
				None,
			)
			.unwrap();

			let data = EvmDataWriter::new_with_selector(Function::Decimals).build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: native_asset_ethereum_address(),
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: true,
			};

			//Act
			let result = CurrencyPrecompile::execute(&mut handle);

			//Assert

			// 12
			let expected_output = hex! {"
				00000000000000000000000000000000 0000000000000000000000000000000C
			"};

			assert_eq!(
				result,
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: expected_output.to_vec()
				})
			);
		});
	}

	#[test]
	fn precompile_for_total_supply_should_work() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let data = EvmDataWriter::new_with_selector(Function::TotalSupply).build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: native_asset_ethereum_address(),
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: true,
			};

			//Act
			let result = CurrencyPrecompile::execute(&mut handle);

			//Assert

			// 950331588000000000
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000D30418B5192A800
			"};

			assert_eq!(
				result,
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: expected_output.to_vec()
				})
			);
		});
	}

	#[test]
	fn precompile_for_balance_of_should_work() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				evm_account(),
				HDX,
				100 * UNITS as i128,
			));

			let data = EvmDataWriter::new_with_selector(Function::BalanceOf)
				.write(Address::from(evm_address()))
				.build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: alice_evm_addr(),
					caller: alice_evm_addr(),
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: true,
			};

			//Act
			let result = CurrencyPrecompile::execute(&mut handle);

			//Assert

			// 100 * UNITS
			let expected_output = hex! {"
				00000000000000000000000000000000 000000000000000000005AF3107A4000
			"};

			assert_eq!(
				result,
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: expected_output.to_vec()
				})
			);
		});
	}

	#[test]
	fn precompile_for_transfer_should_work() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				evm_account(),
				HDX,
				100 * UNITS as i128,
			));

			let data = EvmDataWriter::new_with_selector(Function::Transfer)
				.write(Address::from(evm_address2()))
				.write(U256::from(86u128 * UNITS))
				.build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: evm_address(),
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: false,
			};

			//Act
			let result = CurrencyPrecompile::execute(&mut handle);

			//Assert
			assert_eq!(result.unwrap().exit_status, ExitSucceed::Returned);
			assert_balance!(evm_account2(), HDX, 86u128 * UNITS);
		});
	}

	#[test]
	fn precompile_with_code_transfer_should_work() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			AccountCodes::<hydradx_runtime::Runtime>::insert(
				native_asset_ethereum_address(),
				&hex!["365f5f375f5f365f73bebebebebebebebebebebebebebebebebebebebe5af43d5f5f3e5f3d91602a57fd5bf3"][..],
			);

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				evm_account(),
				HDX,
				100 * UNITS as i128,
			));

			let data = EvmDataWriter::new_with_selector(Function::Transfer)
				.write(Address::from(evm_address2()))
				.write(U256::from(86u128 * UNITS))
				.build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: evm_address(),
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: false,
			};

			//Act
			let result = CurrencyPrecompile::execute(&mut handle);

			//Assert
			assert_eq!(result.unwrap().exit_status, ExitSucceed::Returned);
			assert_balance!(evm_account2(), HDX, 86u128 * UNITS);
		});
	}

	#[test]
	fn precompile_for_currency_approve_allowance_should_fail_as_not_supported() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				evm_account(),
				HDX,
				100 * UNITS as i128,
			));

			let data = EvmDataWriter::new_with_selector(Function::Approve)
				.write(Address::from(evm_address2()))
				.write(U256::from(50u128 * UNITS))
				.build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: native_asset_ethereum_address(),
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: true,
			};

			//Act
			let result = CurrencyPrecompile::execute(&mut handle);

			//Assert
			assert_eq!(
				result,
				Err(PrecompileFailure::Error {
					exit_status: ExitError::Other("not supported".into())
				})
			);
		});
	}

	#[test]
	fn precompile_for_currency_allowance_should_return_zero_for_not_approved_contract() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let data = EvmDataWriter::new_with_selector(Function::Allowance)
				.write(Address::from(evm_address2()))
				.write(Address::from(evm_address()))
				.build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: native_asset_ethereum_address(),
					caller: evm_address(),
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: true,
			};

			//Act
			let result = CurrencyPrecompile::execute(&mut handle);

			//Assert
			assert_eq!(
				result,
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: hex!["0000000000000000000000000000000000000000000000000000000000000000"].to_vec()
				})
			);
		});
	}

	#[test]
	fn precompile_for_currency_allowance_should_return_max_for_approved_contract() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), evm_address(),));

			let data = EvmDataWriter::new_with_selector(Function::Allowance)
				.write(Address::from(evm_address2()))
				.write(Address::from(evm_address()))
				.build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: native_asset_ethereum_address(),
					caller: evm_address(),
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: true,
			};

			//Act
			let result = CurrencyPrecompile::execute(&mut handle);

			//Assert
			assert_eq!(
				result,
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: hex!["00000000000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"].to_vec()
				})
			);
		});
	}

	#[test]
	fn precompile_for_currency_allowance_should_return_zero_for_disapproved_contract() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), evm_address(),));
			let data = EvmDataWriter::new_with_selector(Function::Allowance)
				.write(Address::from(evm_address2()))
				.write(Address::from(evm_address()))
				.build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: native_asset_ethereum_address(),
					caller: evm_address(),
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: true,
			};

			//Act
			assert_ok!(EVMAccounts::disapprove_contract(RuntimeOrigin::root(), evm_address(),));
			let result = CurrencyPrecompile::execute(&mut handle);

			//Assert
			assert_eq!(
				result,
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: hex!["0000000000000000000000000000000000000000000000000000000000000000"].to_vec()
				})
			);
		});
	}

	#[test]
	fn precompile_for_transfer_from_should_fail_for_not_approved_contract() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				evm_account(),
				HDX,
				100 * UNITS as i128,
			));

			let data = EvmDataWriter::new_with_selector(Function::TransferFrom)
				.write(Address::from(evm_address()))
				.write(Address::from(evm_address2()))
				.write(U256::from(50u128 * UNITS))
				.build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: native_asset_ethereum_address(),
					caller: native_asset_ethereum_address(),
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: false,
			};

			//Act & Assert
			assert_noop!(
				CurrencyPrecompile::execute(&mut handle),
				PrecompileFailure::Revert {
					exit_status: Reverted,
					output: "Not approved contract".as_bytes().to_vec()
				}
			);
			assert_balance!(evm_account2(), HDX, 0);
		});
	}

	#[test]
	fn precompile_for_transfer_from_is_allowed_for_approved_contract() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			assert_ok!(EVMAccounts::approve_contract(
				RuntimeOrigin::root(),
				native_asset_ethereum_address(),
			));

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				evm_account(),
				HDX,
				100 * UNITS as i128,
			));

			let data = EvmDataWriter::new_with_selector(Function::TransferFrom)
				.write(Address::from(evm_address()))
				.write(Address::from(evm_address2()))
				.write(U256::from(50u128 * UNITS))
				.build();

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: native_asset_ethereum_address(),
					caller: native_asset_ethereum_address(),
					apparent_value: U256::from(0),
				},
				code_address: native_asset_ethereum_address(),
				is_static: false,
			};

			//Act
			let result = CurrencyPrecompile::execute(&mut handle);
			// Assert
			assert_ok!(result.clone());
			assert_eq!(
				result,
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: hex!["0000000000000000000000000000000000000000000000000000000000000001"].to_vec(),
				})
			);
			assert_balance!(evm_account2(), HDX, 50u128 * UNITS);
		});
	}

	fn account_to_default_evm_address(account_id: &impl Encode) -> EvmAddress {
		let payload = (b"evm:", account_id);
		EvmAddress::from_slice(&payload.using_encoded(blake2_256)[0..20])
	}

	pub fn alice_evm_addr() -> H160 {
		account_to_default_evm_address(&ALICE)
	}

	pub fn alice_substrate_evm_addr() -> AccountId {
		ExtendedAddressMapping::into_account_id(alice_evm_addr())
	}
}

mod chainlink_precompile {
	use super::*;
	use ethabi::ethereum_types::{U128, U256};
	use fp_evm::PrecompileFailure;
	use frame_support::assert_ok;
	use frame_support::sp_runtime::{FixedPointNumber, FixedU128};
	use hex_literal::hex;
	use hydra_dx_math::support::rational::{round_to_rational, Rounding};
	use hydradx_runtime::evm::precompiles::chainlink_adapter;
	use hydradx_runtime::evm::Executor;
	use hydradx_runtime::{
		evm::precompiles::chainlink_adapter::{encode_oracle_address, AggregatorInterface, ChainlinkOraclePrecompile},
		EmaOracle, Inspect, Router, Runtime,
	};
	use hydradx_traits::evm::EVM;
	use hydradx_traits::evm::{CallContext, EvmAddress};
	use hydradx_traits::router::{PoolType, Trade};
	use hydradx_traits::{router::AssetPair, AggregatedPriceOracle, OraclePeriod};
	use pallet_ema_oracle::Price;
	use pallet_lbp::AssetId;
	use primitives::constants::chain::{OMNIPOOL_SOURCE, XYK_SOURCE};

	fn assert_prices_are_same(ema_price: Price, precompile_price: U256, asset_a_decimals: u8, asset_b_decimals: u8) {
		// EMA price does not take into account decimals of the asset. Adjust the price accordingly.
		let decimals_diff = U128::from(asset_a_decimals.abs_diff(asset_b_decimals));
		let adjusted_price = match asset_a_decimals.cmp(&asset_b_decimals) {
			Ordering::Greater => {
				let nominator = U256::from(ema_price.n);
				let denominator = U128::full_mul(ema_price.d.into(), U128::from(10u128).pow(decimals_diff));

				round_to_rational((nominator, denominator), Rounding::Nearest).into()
			}
			Ordering::Less => {
				let nominator = U128::full_mul(ema_price.n.into(), U128::from(10u128).pow(decimals_diff));
				let denominator = U256::from(ema_price.d);

				round_to_rational((nominator, denominator), Rounding::Nearest).into()
			}
			Ordering::Equal => ema_price,
		};

		let decimals = 8u32;
		let fixed_price_int = FixedU128::checked_from_rational(adjusted_price.n, adjusted_price.d)
			.unwrap()
			.checked_mul_int(10_u128.pow(decimals))
			.unwrap();

		pretty_assertions::assert_eq!(fixed_price_int, precompile_price.as_u128());
	}

	#[test]
	fn chainlink_precompile_get_answer_should_work_with_omnipool_source() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			hydradx_run_to_next_block();

			init_omnipool();

			let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);

			assert_ok!(Omnipool::add_token(
				RuntimeOrigin::root(),
				DOT,
				token_price,
				Permill::from_percent(100),
				AccountId::from(BOB),
			));

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DOT,
				5 * UNITS,
				0,
			));

			hydradx_run_to_next_block();

			let hdx_price = EmaOracle::get_price(HDX, LRNA, OraclePeriod::Short, OMNIPOOL_SOURCE)
				.unwrap()
				.0;
			let dot_price = EmaOracle::get_price(DOT, LRNA, OraclePeriod::Short, OMNIPOOL_SOURCE)
				.unwrap()
				.0;
			let ema_price = Price {
				n: hdx_price.n.checked_mul(dot_price.d).unwrap(),
				d: hdx_price.d.checked_mul(dot_price.n).unwrap(),
			};

			let data = EvmDataWriter::new_with_selector(AggregatorInterface::GetAnswer).build();

			let oracle_ethereum_address = encode_oracle_address(HDX, DOT, OraclePeriod::Short, OMNIPOOL_SOURCE);

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: oracle_ethereum_address,
					apparent_value: U256::from(0),
				},
				code_address: oracle_ethereum_address,
				is_static: true,
			};

			//Act
			let PrecompileOutput { output, exit_status } =
				ChainlinkOraclePrecompile::<Runtime>::execute(&mut handle).unwrap();

			//Assert
			pretty_assertions::assert_eq!(exit_status, ExitSucceed::Returned,);

			let asset_a_decimals = AssetRegistry::decimals(HDX).unwrap();
			let asset_b_decimals = AssetRegistry::decimals(DOT).unwrap();
			assert_prices_are_same(
				ema_price,
				U256::from_big_endian(&output),
				asset_a_decimals,
				asset_b_decimals,
			);
		});
	}

	#[test]
	fn chainlink_precompile_latest_answer_should_work_with_omnipool_source() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			hydradx_run_to_next_block();

			init_omnipool();

			let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);

			assert_ok!(Omnipool::add_token(
				RuntimeOrigin::root(),
				DOT,
				token_price,
				Permill::from_percent(100),
				AccountId::from(BOB),
			));

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DOT,
				5 * UNITS,
				0,
			));

			hydradx_run_to_next_block();

			let hdx_price = EmaOracle::get_price(HDX, LRNA, OraclePeriod::Short, OMNIPOOL_SOURCE)
				.unwrap()
				.0;
			let dot_price = EmaOracle::get_price(DOT, LRNA, OraclePeriod::Short, OMNIPOOL_SOURCE)
				.unwrap()
				.0;
			let ema_price = Price {
				n: hdx_price.n.checked_mul(dot_price.d).unwrap(),
				d: hdx_price.d.checked_mul(dot_price.n).unwrap(),
			};

			let data = EvmDataWriter::new_with_selector(AggregatorInterface::LatestAnswer).build();

			let oracle_ethereum_address = encode_oracle_address(HDX, DOT, OraclePeriod::Short, OMNIPOOL_SOURCE);

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: oracle_ethereum_address,
					apparent_value: U256::from(0),
				},
				code_address: oracle_ethereum_address,
				is_static: true,
			};

			//Act
			let PrecompileOutput { output, exit_status } =
				ChainlinkOraclePrecompile::<Runtime>::execute(&mut handle).unwrap();

			//Assert
			pretty_assertions::assert_eq!(exit_status, ExitSucceed::Returned,);

			let asset_a_decimals = AssetRegistry::decimals(HDX).unwrap();
			let asset_b_decimals = AssetRegistry::decimals(DOT).unwrap();
			assert_prices_are_same(
				ema_price,
				U256::from_big_endian(&output),
				asset_a_decimals,
				asset_b_decimals,
			);
		});
	}

	#[test]
	fn chainlink_precompile_get_answer_should_work_with_xyk_source() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			hydradx_run_to_next_block();

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				DOT,
				200 * UNITS as i128,
			));

			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				100 * UNITS,
				DOT,
				200 * UNITS,
			));

			assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), XYK_SOURCE, (HDX, DOT)));

			assert_ok!(XYK::buy(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DOT,
				2 * UNITS,
				200 * UNITS,
				false,
			));

			hydradx_run_to_next_block();

			let ema_price = EmaOracle::get_price(HDX, DOT, OraclePeriod::Short, XYK_SOURCE)
				.unwrap()
				.0;

			let data = EvmDataWriter::new_with_selector(AggregatorInterface::GetAnswer).build();

			let oracle_ethereum_address = encode_oracle_address(HDX, DOT, OraclePeriod::Short, XYK_SOURCE);

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: oracle_ethereum_address,
					apparent_value: U256::from(0),
				},
				code_address: oracle_ethereum_address,
				is_static: true,
			};

			//Act
			let PrecompileOutput { output, exit_status } =
				ChainlinkOraclePrecompile::<Runtime>::execute(&mut handle).unwrap();

			//Assert
			pretty_assertions::assert_eq!(exit_status, ExitSucceed::Returned,);

			let asset_a_decimals = AssetRegistry::decimals(HDX).unwrap();
			let asset_b_decimals = AssetRegistry::decimals(DOT).unwrap();
			assert_prices_are_same(
				ema_price,
				U256::from_big_endian(&output),
				asset_a_decimals,
				asset_b_decimals,
			);
		});
	}

	#[test]
	fn chainlink_precompile_latest_answer_should_work_with_xyk_source() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			hydradx_run_to_next_block();

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				DOT,
				200 * UNITS as i128,
			));

			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				100 * UNITS,
				DOT,
				200 * UNITS,
			));

			assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), XYK_SOURCE, (HDX, DOT)));

			assert_ok!(XYK::buy(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DOT,
				2 * UNITS,
				200 * UNITS,
				false,
			));

			hydradx_run_to_next_block();

			let ema_price = EmaOracle::get_price(HDX, DOT, OraclePeriod::Short, XYK_SOURCE)
				.unwrap()
				.0;

			let data = EvmDataWriter::new_with_selector(AggregatorInterface::LatestAnswer).build();

			let oracle_ethereum_address = encode_oracle_address(HDX, DOT, OraclePeriod::Short, XYK_SOURCE);

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: oracle_ethereum_address,
					apparent_value: U256::from(0),
				},
				code_address: oracle_ethereum_address,
				is_static: true,
			};

			//Act
			let PrecompileOutput { output, exit_status } =
				ChainlinkOraclePrecompile::<Runtime>::execute(&mut handle).unwrap();

			//Assert
			pretty_assertions::assert_eq!(exit_status, ExitSucceed::Returned,);

			let asset_a_decimals = AssetRegistry::decimals(HDX).unwrap();
			let asset_b_decimals = AssetRegistry::decimals(DOT).unwrap();
			assert_prices_are_same(
				ema_price,
				U256::from_big_endian(&output),
				asset_a_decimals,
				asset_b_decimals,
			);
		});
	}

	#[test]
	fn chainlink_precompile_get_answer_should_work_with_routed_pair() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			hydradx_run_to_next_block();

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				DOT,
				200 * UNITS as i128,
			));

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				DAI,
				200 * UNITS as i128,
			));

			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				100 * UNITS,
				DAI,
				200 * UNITS,
			));

			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(ALICE.into()),
				DAI,
				100 * UNITS,
				DOT,
				300 * UNITS,
			));

			assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), XYK_SOURCE, (HDX, DAI)));
			assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), XYK_SOURCE, (DAI, DOT)));

			assert_ok!(XYK::buy(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				2 * UNITS,
				200 * UNITS,
				false,
			));
			assert_ok!(XYK::buy(
				RuntimeOrigin::signed(ALICE.into()),
				DAI,
				DOT,
				2 * UNITS,
				200 * UNITS,
				false,
			));

			// set route
			let route = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DAI,
					asset_out: DOT,
				},
			];

			hydradx_run_to_next_block();

			assert_ok!(Router::set_route(
				RuntimeOrigin::signed(ALICE.into()),
				AssetPair::new(HDX, DOT),
				route.try_into().unwrap()
			));

			let dai_price = EmaOracle::get_price(HDX, DAI, OraclePeriod::Short, XYK_SOURCE)
				.unwrap()
				.0;
			let dot_price = EmaOracle::get_price(DAI, DOT, OraclePeriod::Short, XYK_SOURCE)
				.unwrap()
				.0;
			let ema_price = Price {
				n: dai_price.n.checked_mul(dot_price.n).unwrap(),
				d: dai_price.d.checked_mul(dot_price.d).unwrap(),
			};

			let data = EvmDataWriter::new_with_selector(AggregatorInterface::GetAnswer).build();

			let oracle_ethereum_address = encode_oracle_address(HDX, DOT, OraclePeriod::Short, [0; 8]);

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: oracle_ethereum_address,
					apparent_value: U256::from(0),
				},
				code_address: oracle_ethereum_address,
				is_static: true,
			};

			//Act
			let PrecompileOutput { output, exit_status } =
				ChainlinkOraclePrecompile::<Runtime>::execute(&mut handle).unwrap();

			//Assert
			pretty_assertions::assert_eq!(exit_status, ExitSucceed::Returned,);

			let asset_a_decimals = AssetRegistry::decimals(HDX).unwrap();
			let asset_b_decimals = AssetRegistry::decimals(DOT).unwrap();
			assert_prices_are_same(
				ema_price,
				U256::from_big_endian(&output),
				asset_a_decimals,
				asset_b_decimals,
			);
		});
	}

	#[test]
	fn chainlink_precompile_latest_answer_should_work_with_routed_pair() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			hydradx_run_to_next_block();

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				DOT,
				200 * UNITS as i128,
			));

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				DAI,
				200 * UNITS as i128,
			));

			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				100 * UNITS,
				DAI,
				200 * UNITS,
			));

			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(ALICE.into()),
				DAI,
				100 * UNITS,
				DOT,
				300 * UNITS,
			));

			assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), XYK_SOURCE, (HDX, DAI)));
			assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), XYK_SOURCE, (DAI, DOT)));

			assert_ok!(XYK::buy(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				2 * UNITS,
				200 * UNITS,
				false,
			));
			assert_ok!(XYK::buy(
				RuntimeOrigin::signed(ALICE.into()),
				DAI,
				DOT,
				2 * UNITS,
				200 * UNITS,
				false,
			));

			// set route
			let route = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DAI,
					asset_out: DOT,
				},
			];

			hydradx_run_to_next_block();

			assert_ok!(Router::set_route(
				RuntimeOrigin::signed(ALICE.into()),
				AssetPair::new(HDX, DOT),
				route.try_into().unwrap()
			));

			let dai_price = EmaOracle::get_price(HDX, DAI, OraclePeriod::Short, XYK_SOURCE)
				.unwrap()
				.0;
			let dot_price = EmaOracle::get_price(DAI, DOT, OraclePeriod::Short, XYK_SOURCE)
				.unwrap()
				.0;
			let ema_price = Price {
				n: dai_price.n.checked_mul(dot_price.n).unwrap(),
				d: dai_price.d.checked_mul(dot_price.d).unwrap(),
			};

			let data = EvmDataWriter::new_with_selector(AggregatorInterface::LatestAnswer).build();

			let oracle_ethereum_address = encode_oracle_address(HDX, DOT, OraclePeriod::Short, [0; 8]);

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: oracle_ethereum_address,
					apparent_value: U256::from(0),
				},
				code_address: oracle_ethereum_address,
				is_static: true,
			};

			//Act
			let PrecompileOutput { output, exit_status } =
				ChainlinkOraclePrecompile::<Runtime>::execute(&mut handle).unwrap();

			//Assert
			pretty_assertions::assert_eq!(exit_status, ExitSucceed::Returned,);

			let asset_a_decimals = AssetRegistry::decimals(HDX).unwrap();
			let asset_b_decimals = AssetRegistry::decimals(DOT).unwrap();
			assert_prices_are_same(
				ema_price,
				U256::from_big_endian(&output),
				asset_a_decimals,
				asset_b_decimals,
			);
		});
	}

	#[test]
	fn chainlink_precompile_should_return_error_when_oracle_not_available() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			assert!(EmaOracle::get_price(HDX, DOT, OraclePeriod::Short, XYK_SOURCE).is_err());

			let data = EvmDataWriter::new_with_selector(AggregatorInterface::GetAnswer).build();

			let oracle_ethereum_address = encode_oracle_address(HDX, DOT, OraclePeriod::Short, XYK_SOURCE);

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: oracle_ethereum_address,
					apparent_value: U256::from(0),
				},
				code_address: oracle_ethereum_address,
				is_static: true,
			};

			//Act
			let result = ChainlinkOraclePrecompile::<Runtime>::execute(&mut handle);

			//Assert
			pretty_assertions::assert_eq!(
				result,
				Err(PrecompileFailure::Error {
					exit_status: ExitError::Other("Price not available".into()),
				})
			);
		});
	}

	#[test]
	fn chainlink_runtime_rpc_should_work() {
		use hydradx_runtime::evm::precompiles::chainlink_adapter::runtime_api::runtime_decl_for_chainlink_adapter_api::ChainlinkAdapterApiV1;

		TestNet::reset();

		Hydra::execute_with(|| {
			pretty_assertions::assert_eq!(
				Runtime::encode_oracle_address(4, 5, OraclePeriod::TenMinutes, OMNIPOOL_SOURCE),
				encode_oracle_address(4, 5, OraclePeriod::TenMinutes, OMNIPOOL_SOURCE)
			);

			pretty_assertions::assert_eq!(
				Runtime::decode_oracle_address(H160::from(hex!("000001026f6d6e69706f6f6c0000000400000005"))),
				chainlink_adapter::decode_oracle_address(H160::from(hex!("000001026f6d6e69706f6f6c0000000400000005")))
			);
		});
	}

	fn prices_should_be_comparable_with_dia(asset: AssetId, reference_oracle: EvmAddress) {
		TestNet::reset();
		// ./target/release/scraper save-storage --pallet AssetRegistry EmaOracle Router EVM --uri wss://rpc.hydradx.cloud --at 0x8c5aaf89eb976d1b1f9c12e517bf33bbc1f34c05b76811a288fd6e7912ff2bbc
		hydra_live_ext("evm-snapshot/router").execute_with(|| {
			let base = 10;
			let route = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: asset,
					asset_out: 102,
				},
				Trade {
					pool: PoolType::Stableswap(102),
					asset_in: 102,
					asset_out: base,
				},
			];
			assert_ok!(Router::force_insert_route(
				RuntimeOrigin::root(),
				AssetPair {
					asset_in: asset,
					asset_out: base
				},
				BoundedVec::truncate_from(route)
			));

			let input = EvmDataWriter::new_with_selector(AggregatorInterface::LatestAnswer).build();

			// kinda weird that asset order is reversed, would expect WBTC/USDT instead
			let address = encode_oracle_address(base, asset, OraclePeriod::TenMinutes, [0; 8]);
			let mut handle = MockHandle {
				input: input.clone(),
				context: Context {
					address,
					caller: Default::default(),
					apparent_value: Default::default(),
				},
				code_address: address,
				is_static: true,
			};
			let PrecompileOutput { output, .. } =
				ChainlinkOraclePrecompile::<hydradx_runtime::Runtime>::execute(&mut handle).unwrap();
			let precompile_price = U256::from(output.as_slice());

			let call_result = Executor::<Runtime>::view(
				CallContext {
					contract: reference_oracle,
					sender: Default::default(),
					origin: Default::default(),
				},
				input,
				100_000,
			);
			let dia_price = U256::from(call_result.value.as_slice());

			// Prices doesn't need to be exactly same, but comparable within 5%
			let tolerance = dia_price
				.checked_mul(5.into())
				.unwrap()
				.checked_div(100.into())
				.unwrap();
			let price_diff = dia_price.abs_diff(precompile_price);
			assert!(price_diff < tolerance);
		});
	}

	#[test]
	fn dot_price_should_be_comparable() {
		prices_should_be_comparable_with_dia(5, hex!["FBCa0A6dC5B74C042DF23025D99ef0F1fcAC6702"].into())
		// 336479050
	}

	#[test]
	fn bitcoin_price_should_be_comparable() {
		prices_should_be_comparable_with_dia(19, hex!["eDD9A7C47A9F91a0F2db93978A88844167B4a04f"].into())
	}

	#[test]
	fn chainlink_decimasl_should_return_8() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let data = EvmDataWriter::new_with_selector(AggregatorInterface::Decimals).build();

			let oracle_ethereum_address = encode_oracle_address(HDX, DOT, OraclePeriod::Short, OMNIPOOL_SOURCE);

			let mut handle = MockHandle {
				input: data,
				context: Context {
					address: evm_address(),
					caller: oracle_ethereum_address,
					apparent_value: U256::from(0),
				},
				code_address: oracle_ethereum_address,
				is_static: true,
			};

			//Act
			let PrecompileOutput { output, exit_status } =
				ChainlinkOraclePrecompile::<Runtime>::execute(&mut handle).unwrap();

			//Assert
			pretty_assertions::assert_eq!(exit_status, ExitSucceed::Returned,);

			let expected_decimals: u8 = 8;
			let r: u8 = U256::from(output.as_slice()).try_into().unwrap();
			pretty_assertions::assert_eq!(r, expected_decimals);
		});
	}
}

mod contract_deployment {
	use super::*;
	use frame_support::assert_noop;
	use pretty_assertions::assert_eq;

	#[test]
	fn create_contract_from_runtime_rpc_should_be_rejected_if_address_is_not_whitelisted() {
		TestNet::reset();

		Hydra::execute_with(|| {
			assert_noop!(
				hydradx_runtime::Runtime::create(
					evm_address(),
					vec![0, 1, 1, 0],
					U256::zero(),
					U256::from(100000u64),
					None,
					None,
					None,
					false,
					None,
				),
				pallet_evm_accounts::Error::<hydradx_runtime::Runtime>::AddressNotWhitelisted
			);
		});
	}

	#[test]
	fn create_contract_from_runtime_rpc_should_be_accepted_if_address_is_whitelisted() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE));
			assert_ok!(EVMAccounts::add_contract_deployer(RuntimeOrigin::root(), evm_address));

			assert_ok!(hydradx_runtime::Runtime::create(
				evm_address,
				vec![0, 1, 1, 0],
				U256::zero(),
				U256::from(100000u64),
				None,
				None,
				None,
				false,
				None,
			));
		});
	}
}

mod account_marking {
	use super::*;
	use frame_support::assert_ok;
	use pretty_assertions::assert_eq;

	#[test]
	fn account_should_be_marked_and_sufficients_inreased_on_first_evm_transaction() {
		TestNet::reset();

		Hydra::execute_with(|| {
			// Arrange
			let evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE));
			let account_id = EVMAccounts::account_id(evm_address);

			let initial_sufficients = frame_system::Pallet::<hydradx_runtime::Runtime>::sufficients(&account_id);

			assert_eq!(hydradx_runtime::EVMAccounts::marked_evm_accounts(&account_id), None);

			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				account_id.clone(),
				WETH,
				100000000 * UNITS as i128,
			));

			let data =
				hex!["4d0045544800d1820d45118d78d091e685490c674d7596e62d1f0000000000000000140000000f0000c16ff28623"]
					.to_vec();

			// Act
			assert_ok!(EVM::call(
				evm_signed_origin(evm_address),
				evm_address,
				DISPATCH_ADDR,
				data,
				U256::from(0),
				1000000,
				gas_price(),
				None,
				Some(U256::zero()),
				[].into()
			));

			assert!(hydradx_runtime::EVMAccounts::marked_evm_accounts(&account_id).is_some());

			let final_sufficients = frame_system::Pallet::<hydradx_runtime::Runtime>::sufficients(&account_id);
			assert_eq!(final_sufficients, initial_sufficients + 1);
		});
	}

	#[test]
	fn marking_should_be_idempotent_across_multiple_transactions() {
		TestNet::reset();

		Hydra::execute_with(|| {
			// Arrange
			let evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE));
			let account_id = EVMAccounts::account_id(evm_address);

			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				account_id.clone(),
				WETH,
				1000 * UNITS as i128,
			));

			let data =
				hex!["4d0045544800d1820d45118d78d091e685490c674d7596e62d1f0000000000000000140000000f0000c16ff28623"]
					.to_vec();

			// Act - Execute first EVM transaction
			assert_ok!(EVM::call(
				evm_signed_origin(evm_address),
				evm_address,
				DISPATCH_ADDR,
				data.clone(),
				U256::from(0),
				1000000,
				gas_price(),
				None,
				Some(U256::zero()),
				[].into()
			));

			let sufficients_after_first = frame_system::Pallet::<hydradx_runtime::Runtime>::sufficients(&account_id);

			// Execute second EVM transaction
			assert_ok!(EVM::call(
				evm_signed_origin(evm_address),
				evm_address,
				DISPATCH_ADDR,
				data.clone(),
				U256::from(0),
				1000000,
				gas_price(),
				None,
				Some(U256::from(1)),
				[].into()
			));

			let sufficients_after_second = frame_system::Pallet::<hydradx_runtime::Runtime>::sufficients(&account_id);

			// Execute third EVM transaction
			assert_ok!(EVM::call(
				evm_signed_origin(evm_address),
				evm_address,
				DISPATCH_ADDR,
				data,
				U256::from(0),
				1000000,
				gas_price(),
				None,
				Some(U256::from(2)),
				[].into()
			));

			let sufficients_after_third = frame_system::Pallet::<hydradx_runtime::Runtime>::sufficients(&account_id);

			// Assert - Sufficients should only increment once, not for each transaction
			assert_eq!(sufficients_after_first, sufficients_after_second);
			assert_eq!(sufficients_after_second, sufficients_after_third);

			// Account should still be marked
			assert!(hydradx_runtime::EVMAccounts::marked_evm_accounts(&account_id).is_some());
		});
	}

	#[test]
	fn multiple_different_accounts_can_be_marked_independently() {
		TestNet::reset();

		Hydra::execute_with(|| {
			// Arrange - Set up two different accounts
			let alice_evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE));
			let alice_account_id: AccountId = EVMAccounts::account_id(alice_evm_address);

			let bob_evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(BOB));
			let bob_account_id: AccountId = EVMAccounts::account_id(bob_evm_address);

			assert!(hydradx_runtime::EVMAccounts::marked_evm_accounts(&alice_account_id).is_none());
			assert!(hydradx_runtime::EVMAccounts::marked_evm_accounts(&bob_account_id).is_none());

			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				alice_account_id.clone(),
				WETH,
				100 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				bob_account_id.clone(),
				WETH,
				100 * UNITS as i128,
			));

			let data =
				hex!["4d0045544800d1820d45118d78d091e685490c674d7596e62d1f0000000000000000140000000f0000c16ff28623"]
					.to_vec();

			let alice_initial_sufficients =
				frame_system::Pallet::<hydradx_runtime::Runtime>::sufficients(&alice_account_id);
			let bob_initial_sufficients =
				frame_system::Pallet::<hydradx_runtime::Runtime>::sufficients(&bob_account_id);

			// Act
			assert_ok!(EVM::call(
				evm_signed_origin(alice_evm_address),
				alice_evm_address,
				DISPATCH_ADDR,
				data.clone(),
				U256::from(0),
				1000000,
				gas_price(),
				None,
				Some(U256::zero()),
				[].into()
			));

			// Assert
			assert!(hydradx_runtime::EVMAccounts::marked_evm_accounts(&alice_account_id).is_some());
			assert!(hydradx_runtime::EVMAccounts::marked_evm_accounts(&bob_account_id).is_none());

			let alice_sufficients_after =
				frame_system::Pallet::<hydradx_runtime::Runtime>::sufficients(&alice_account_id);
			assert_eq!(alice_sufficients_after, alice_initial_sufficients + 1);

			// Act - Bob makes an EVM transaction
			assert_ok!(EVM::call(
				evm_signed_origin(bob_evm_address),
				bob_evm_address,
				DISPATCH_ADDR,
				data,
				U256::from(0),
				1000000,
				gas_price(),
				None,
				Some(U256::zero()),
				[].into()
			));

			// Assert
			assert!(hydradx_runtime::EVMAccounts::marked_evm_accounts(&alice_account_id).is_some());
			assert!(hydradx_runtime::EVMAccounts::marked_evm_accounts(&bob_account_id).is_some());

			let bob_sufficients_after = frame_system::Pallet::<hydradx_runtime::Runtime>::sufficients(&bob_account_id);
			assert_eq!(bob_sufficients_after, bob_initial_sufficients + 1);

			let alice_sufficients_final =
				frame_system::Pallet::<hydradx_runtime::Runtime>::sufficients(&alice_account_id);
			assert_eq!(alice_sufficients_final, alice_sufficients_after);
		});
	}

	#[test]
	fn bound_account_should_not_be_marked_on_evm_transaction_because_only_truncated_evm_accounts_marked() {
		TestNet::reset();

		Hydra::execute_with(|| {
			// Arrange
			init_omnipool_with_oracle_for_block_10();
			let account_id: AccountId = Into::<AccountId>::into(ALICE);

			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				account_id.clone()
			)));

			let evm_address = EVMAccounts::evm_address(&account_id);

			assert!(hydradx_runtime::EVMAccounts::marked_evm_accounts(&account_id).is_none());

			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				account_id.clone(),
				WETH,
				100000 * UNITS as i128,
			));

			let data =
				hex!["4d0045544800d1820d45118d78d091e685490c674d7596e62d1f0000000000000000140000000f0000c16ff28623"]
					.to_vec();

			let initial_sufficients = frame_system::Pallet::<hydradx_runtime::Runtime>::sufficients(&account_id);
			let (gas_price, _) = hydradx_runtime::DynamicEvmFee::min_gas_price();

			// Act
			assert_ok!(EVM::call(
				evm_signed_origin(evm_address),
				evm_address,
				DISPATCH_ADDR,
				data,
				U256::from(0),
				1000000,
				gas_price * 10,
				None,
				Some(U256::zero()),
				[].into()
			));

			// Assert
			assert!(hydradx_runtime::EVMAccounts::marked_evm_accounts(&account_id).is_none());

			let final_sufficients = frame_system::Pallet::<hydradx_runtime::Runtime>::sufficients(&account_id);
			assert_eq!(final_sufficients, initial_sufficients);
		});
	}

	#[test]
	fn evm_should_not_be_reaped_when_only_erc20_left() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			let user_evm_address = alith_evm_address();
			let user_secret_key = alith_secret_key();
			let user_acc = MockAccount::new(alith_truncated_account());

			// Execute multiple EVM transactions to increase the nonce
			let data =
				hex!["4d0045544800d1820d45118d78d091e685490c674d7596e62d1f0000000000000000140000000f0000c16ff28623"]
					.to_vec();

			// Fund account with WETH for EVM gas fees
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				WETH,
				10000 * UNITS as i128,
			));

			let state = frame_system::Pallet::<Runtime>::account(&user_acc.address());
			assert_eq!(state.providers, 1);
			assert_eq!(state.sufficients, 0);
			assert_eq!(state.nonce, 0);

			let (gas_price, _) = hydradx_runtime::DynamicEvmFee::min_gas_price();

			// Execute first EVM transaction (nonce 0)
			assert_ok!(EVM::call(
				evm_signed_origin(user_evm_address),
				user_evm_address,
				DISPATCH_ADDR,
				data.clone(),
				U256::from(0),
				1000000,
				gas_price * 10,
				None,
				Some(U256::zero()),
				[].into()
			));

			// Execute second EVM transaction (nonce 1)
			assert_ok!(EVM::call(
				evm_signed_origin(user_evm_address),
				user_evm_address,
				DISPATCH_ADDR,
				data.clone(),
				U256::from(0),
				1000000,
				gas_price * 10,
				None,
				Some(U256::from(1)),
				[].into()
			));

			// Execute third EVM transaction (nonce 2)
			assert_ok!(EVM::call(
				evm_signed_origin(user_evm_address),
				user_evm_address,
				DISPATCH_ADDR,
				data,
				U256::from(0),
				1000000,
				gas_price * 10,
				None,
				Some(U256::from(2)),
				[].into()
			));

			// Verify the nonce and sufficients were incremented through EVM transactions
			let state = frame_system::Pallet::<Runtime>::account(&user_acc.address());
			assert_eq!(state.providers, 1);
			assert_eq!(state.sufficients, 1);
			assert_eq!(state.nonce, 3,);

			//Deploy and use erc20

			let contract = crate::erc20::deploy_token_contract();
			let erc20 = crate::erc20::bind_erc20(contract);
			let balance = Currencies::free_balance(erc20, &ALICE.into());
			let erc20_balance = 2000000000000000;
			assert_eq!(erc20_balance, 2000000000000000);
			assert_ok!(<Erc20Currency<Runtime> as ERC20>::transfer(
				CallContext {
					contract: contract,
					sender: crate::erc20::deployer(),
					origin: crate::erc20::deployer()
				},
				user_evm_address,
				erc20_balance
			));

			assert_ok!(Currencies::transfer(
				hydradx_runtime::RuntimeOrigin::signed(alith_evm_account()),
				hydradx_runtime::Omnipool::protocol_account(),
				erc20,
				erc20_balance / 2
			));
			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				erc20,
				FixedU128::from_rational(1, 2),
				Permill::from_percent(100),
				AccountId::from(alith_evm_account()),
			));
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(alith_evm_account()),
				erc20,
				0,
				erc20_balance / 100,
				Balance::MIN
			));
			hydradx_run_to_next_block();

			// Ensure Alith starts clean-ish
			// Remove WETH if any (to ensure account can die)
			let weth_balance = user_acc.balance(WETH);
			if weth_balance > 0 {
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					user_acc.address(),
					WETH,
					-(weth_balance as i128),
				));
			}

			// Fund with HDX
			let initial_amount = 1000 * UNITS;
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				initial_amount as i128,
			));

			let state = frame_system::Pallet::<Runtime>::account(&user_acc.address());
			assert_eq!(state.providers, 1);

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(alith_evm_account()),
				0,
				erc20,
				UNITS,
				Balance::MIN
			));

			// Construct transfer_all call
			let transfer_all = RuntimeCall::Balances(pallet_balances::Call::transfer_all {
				dest: BOB.into(),
				keep_alive: false,
			});

			let gas_limit = 1_000_000;
			let deadline = U256::from(u64::MAX);

			assert_ok!(MultiTransactionPayment::add_currency(
				RuntimeOrigin::root(),
				erc20,
				FixedU128::from_rational(1, 4)
			));

			assert_ok!(MultiTransactionPayment::set_currency(
				RuntimeOrigin::signed(user_acc.address().clone()),
				erc20,
			));
			let permit =
				pallet_evm_precompile_call_permit::CallPermitPrecompile::<hydradx_runtime::Runtime>::generate_permit(
					CALLPERMIT,
					user_evm_address,
					DISPATCH_ADDR,
					U256::from(0),
					transfer_all.encode(),
					gas_limit,
					U256::zero(),
					deadline,
				);

			let secret_key = SecretKey::parse(&user_secret_key).unwrap();
			let message = Message::parse(&permit);
			let (rs, v) = sign(&message, &secret_key);

			// Dispatch permit
			assert_ok!(MultiTransactionPayment::dispatch_permit(
				hydradx_runtime::RuntimeOrigin::none(),
				user_evm_address,
				DISPATCH_ADDR,
				U256::from(0),
				transfer_all.encode(),
				gas_limit,
				deadline,
				v.serialize(),
				H256::from(rs.r.b32()),
				H256::from(rs.s.b32()),
			));

			let free_hdx = Currencies::free_balance(HDX, &user_acc.address());
			assert_eq!(free_hdx, 0);

			let state = frame_system::Pallet::<Runtime>::account(&user_acc.address());
			assert_eq!(state.providers, 0);
			assert_eq!(state.sufficients, 1);
			assert_eq!(state.nonce, 3);
		});
	}

	#[test]
	fn evm_should_not_be_reaped_when_only_native_left() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			let user_evm_address = alith_evm_address();
			let user_secret_key = alith_secret_key();
			let user_acc = MockAccount::new(alith_truncated_account());

			//Deploy and use erc20
			let contract = crate::erc20::deploy_token_contract();
			let erc20 = crate::erc20::bind_erc20(contract);
			let balance = Currencies::free_balance(erc20, &ALICE.into());
			let erc20_balance = 2000000000000000;
			assert_eq!(erc20_balance, 2000000000000000);
			assert_ok!(<Erc20Currency<Runtime> as ERC20>::transfer(
				CallContext {
					contract: contract,
					sender: crate::erc20::deployer(),
					origin: crate::erc20::deployer()
				},
				user_evm_address,
				erc20_balance
			));

			assert_ok!(Currencies::transfer(
				hydradx_runtime::RuntimeOrigin::signed(alith_evm_account()),
				hydradx_runtime::Omnipool::protocol_account(),
				erc20,
				erc20_balance / 2
			));
			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				erc20,
				FixedU128::from_rational(1, 2),
				Permill::from_percent(100),
				AccountId::from(alith_evm_account()),
			));
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(alith_evm_account()),
				erc20,
				0,
				erc20_balance / 100,
				Balance::MIN
			));
			hydradx_run_to_next_block();

			// Ensure Alith starts clean-ish
			// Remove WETH if any (to ensure account can die)
			let weth_balance = user_acc.balance(WETH);
			if weth_balance > 0 {
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					user_acc.address(),
					WETH,
					-(weth_balance as i128),
				));
			}

			// Fund with HDX
			let initial_amount = 1000 * UNITS;
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				user_acc.address(),
				HDX,
				initial_amount as i128,
			));

			let state = frame_system::Pallet::<Runtime>::account(&user_acc.address());
			assert_eq!(state.providers, 1);

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(alith_evm_account()),
				0,
				erc20,
				UNITS,
				Balance::MIN
			));

			let erc20_balance = Currencies::free_balance(erc20, &user_acc.address());
			// Construct transfer_all call
			let transfer_all = RuntimeCall::Currencies(pallet_currencies::Call::transfer {
				dest: BOB.into(),
				currency_id: erc20,
				amount: erc20_balance,
			});

			let gas_limit = 1_000_000;
			let deadline = U256::from(u64::MAX);

			assert_ok!(MultiTransactionPayment::add_currency(
				RuntimeOrigin::root(),
				erc20,
				FixedU128::from_rational(1, 4)
			));

			assert_ok!(MultiTransactionPayment::set_currency(
				RuntimeOrigin::signed(user_acc.address().clone()),
				erc20,
			));
			let permit =
				pallet_evm_precompile_call_permit::CallPermitPrecompile::<hydradx_runtime::Runtime>::generate_permit(
					CALLPERMIT,
					user_evm_address,
					DISPATCH_ADDR,
					U256::from(0),
					transfer_all.encode(),
					gas_limit,
					U256::zero(),
					deadline,
				);

			let secret_key = SecretKey::parse(&user_secret_key).unwrap();
			let message = Message::parse(&permit);
			let (rs, v) = sign(&message, &secret_key);

			// Dispatch permit
			assert_ok!(MultiTransactionPayment::dispatch_permit(
				hydradx_runtime::RuntimeOrigin::none(),
				user_evm_address,
				DISPATCH_ADDR,
				U256::from(0),
				transfer_all.encode(),
				gas_limit,
				deadline,
				v.serialize(),
				H256::from(rs.r.b32()),
				H256::from(rs.s.b32()),
			));

			let free_hdx = Currencies::free_balance(HDX, &user_acc.address());
			assert!(free_hdx > 0);

			let state = frame_system::Pallet::<Runtime>::account(&user_acc.address());
			assert_eq!(state.providers, 1);
			assert_eq!(state.sufficients, 1);
			assert_eq!(state.nonce, 0);
		});
	}
}

#[test]
fn dispatch_should_work_with_remark() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let mut handle = create_dispatch_handle(hex!["0107081337"].to_vec());

		//Act
		let prec = HydraDXPrecompiles::<hydradx_runtime::Runtime>::new();
		let result = prec.execute(&mut handle);

		//Assert
		assert_eq!(
			result.unwrap(),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Stopped,
				output: Default::default(),
			})
		)
	});
}

#[test]
fn dispatch_should_work_with_transfer() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Set up to idle state where the chain is not utilized at all
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())));

		let evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE));
		init_omnipool_with_oracle_for_block_10();
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			WETH,
			(100 * UNITS * 1_000_000) as i128,
		));
		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			RuntimeOrigin::signed(ALICE.into()),
			WETH,
		));

		//Arrange
		let data = hex!["4d0045544800d1820d45118d78d091e685490c674d7596e62d1f0000000000000000140000000f0000c16ff28623"]
			.to_vec();
		let balance = Tokens::free_balance(WETH, &AccountId::from(ALICE));

		let (gas_price, _) = hydradx_runtime::DynamicEvmFee::min_gas_price();

		//Act
		assert_ok!(EVM::call(
			RuntimeOrigin::signed(ALICE.into()),
			evm_address,
			DISPATCH_ADDR,
			data,
			U256::from(0),
			1000000,
			gas_price * 10,
			None,
			Some(U256::zero()),
			[].into()
		));

		//Assert
		assert!(Tokens::free_balance(WETH, &AccountId::from(ALICE)) < balance - 10u128.pow(16));
	});
}

#[test]
fn dispatch_should_work_with_buying_insufficient_asset() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Set up to idle state where the chain is not utilized at all
		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())));

		//Create inssufficient asset
		let altcoin = with_transaction::<u32, DispatchError, _>(|| {
			let name = b"ALTTKN".to_vec();
			let altcoin = AssetRegistry::register_insufficient_asset(
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

			TransactionOutcome::Commit(Ok(altcoin))
		})
		.unwrap();

		create_xyk_pool_with_amounts(altcoin, 1000000 * UNITS, HDX, 1000000 * UNITS);
		init_omnipool_with_oracle_for_block_10();

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			currency_precompile::alice_substrate_evm_addr(),
			WETH,
			(100 * UNITS * 1_000_000) as i128,
		));
		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			RuntimeOrigin::signed(ALICE.into()),
			WETH,
		));

		let swap_route = vec![
			Trade {
				pool: PoolType::Omnipool,
				asset_in: WETH,
				asset_out: HDX,
			},
			Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: altcoin,
			},
		];
		let router_swap = RuntimeCall::Router(pallet_route_executor::Call::buy {
			asset_in: WETH,
			asset_out: altcoin,
			amount_out: UNITS,
			max_amount_in: u128::MAX,
			route: BoundedVec::truncate_from(swap_route),
		});

		//Arrange
		let data = router_swap.encode();
		let (gas_price, _) = hydradx_runtime::DynamicEvmFee::min_gas_price();

		hydradx_finalize_block(); //We do this to simulate that we don't have any prices in multi-payment-pallet, but the prices can be still calculated based on onchain route

		let init_balance = Tokens::free_balance(altcoin, &currency_precompile::alice_substrate_evm_addr());
		assert_eq!(init_balance, 0);

		// Act
		assert_ok!(EVM::call(
			evm_signed_origin(currency_precompile::alice_evm_addr()),
			currency_precompile::alice_evm_addr(),
			DISPATCH_ADDR,
			data,
			U256::from(0),
			1000000,
			gas_price * 10,
			None,
			Some(U256::zero()),
			[].into()
		));

		//EVM call passes even when the substrate tx fails, so we need to check if the tx is executed
		expect_hydra_last_events(vec![pallet_evm::Event::Executed { address: DISPATCH_ADDR }.into()]);
		let new_balance = Tokens::free_balance(altcoin, &currency_precompile::alice_substrate_evm_addr());
		assert_eq!(new_balance, UNITS);
	});
}

#[test]
fn dispatch_transfer_should_not_work_with_insufficient_fees() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let data = hex!["4d0045544800d1820d45118d78d091e685490c674d7596e62d1f0000000000000000140000000f0000c16ff28623"]
			.to_vec();
		let insufficient_gas_price = gas_price() - U256::one();

		//Act
		let call = EVM::call(
			evm_signed_origin(evm_address()),
			evm_address(),
			DISPATCH_ADDR,
			data,
			U256::from(0),
			1000000,
			insufficient_gas_price,
			None,
			Some(U256::zero()),
			[].into(),
		);

		//Assert
		call.expect_err("Expected GasPriceTooLow error");
	});
}

#[test]
fn dispatch_should_respect_call_filter() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		//Arrange
		let amount = 10u128.pow(16);
		let gas_limit = 1000000;
		let transfer_call = RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: ALICE.into(),
			currency_id: WETH,
			amount,
		});
		assert!(CallFilter::contains(&transfer_call));
		assert_ok!(TransactionPause::pause_transaction(
			RuntimeOrigin::root(),
			b"Tokens".to_vec(),
			b"transfer".to_vec()
		));
		assert!(!CallFilter::contains(&transfer_call));

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			currency_precompile::alice_substrate_evm_addr(),
			WETH,
			(100 * UNITS * 1_000_000) as i128,
		));
		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			evm_signed_origin(currency_precompile::alice_evm_addr()),
			WETH,
		));
		let balance = Tokens::free_balance(WETH, &currency_precompile::alice_substrate_evm_addr());

		let (gas_price, _) = hydradx_runtime::DynamicEvmFee::min_gas_price();
		//Act
		assert_ok!(EVM::call(
			evm_signed_origin(currency_precompile::alice_evm_addr()),
			currency_precompile::alice_evm_addr(),
			DISPATCH_ADDR,
			transfer_call.encode(),
			U256::from(0),
			gas_limit,
			gas_price * 10,
			None,
			Some(U256::zero()),
			[].into(),
		));

		//Assert
		let new_balance = Tokens::free_balance(WETH, &currency_precompile::alice_substrate_evm_addr());
		assert!(new_balance < balance, "fee wasn't charged");
		assert!(new_balance > balance - amount, "more than fee was taken from account");
		assert_eq!(
			new_balance,
			balance - (U256::from(gas_limit) * gas_price).as_u128(),
			"gas limit was not charged"
		);
		assert_eq!(
			HydraDXPrecompiles::<hydradx_runtime::Runtime>::new()
				.execute(&mut create_dispatch_handle(transfer_call.encode()))
				.unwrap(),
			Err(PrecompileFailure::Error {
				exit_status: ExitError::Other(Cow::from("dispatch execution failed: CallFiltered"))
			})
		);
	});
}

#[test]
fn compare_fee_in_eth_between_evm_and_native_omnipool_calls() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let fee_currency = WETH;
		let evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())));

		init_omnipool_with_oracle_for_block_10();

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			WETH,
			(10_000_000 * UNITS) as i128,
		));
		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			RuntimeOrigin::signed(ALICE.into()),
			fee_currency,
		));

		// give alice evm addr some DOT to sell in omnipool
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			DOT,
			(10 * UNITS) as i128,
		));

		let treasury_currency_balance = Currencies::free_balance(fee_currency, &Treasury::account_id());
		let alice_currency_balance = Currencies::free_balance(fee_currency, &AccountId::from(ALICE));

		//Act
		let omni_sell = RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
			asset_in: DOT,
			asset_out: HDX,
			amount: 10_000_000_000,
			min_buy_amount: 0,
		});

		let gas_limit = 1_000_000;
		let (gas_price, _) = hydradx_runtime::DynamicEvmFee::min_gas_price();

		//Execute omnipool sell via EVM
		assert_ok!(EVM::call(
			RuntimeOrigin::signed(ALICE.into()),
			evm_address,
			DISPATCH_ADDR,
			omni_sell.encode(),
			U256::from(0),
			gas_limit,
			gas_price * 10,
			None,
			Some(U256::zero()),
			[].into(),
		));

		let new_treasury_currency_balance = Currencies::free_balance(fee_currency, &Treasury::account_id());
		let new_alice_currency_balance = Currencies::free_balance(fee_currency, &AccountId::from(ALICE));
		let evm_fee = alice_currency_balance - new_alice_currency_balance;
		let treasury_evm_fee = new_treasury_currency_balance - treasury_currency_balance;
		assert_eq!(treasury_evm_fee, evm_fee);

		//Pre dispatch the native omnipool call - so withdrawing only the fees for the execution
		let info = omni_sell.get_dispatch_info();
		let len: usize = 146;
		let pre = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
			.pre_dispatch(&AccountId::from(ALICE), &omni_sell, &info, len);
		assert_ok!(&pre);

		let alice_currency_balance_pre_dispatch = Currencies::free_balance(fee_currency, &AccountId::from(ALICE));
		let native_fee = new_alice_currency_balance - alice_currency_balance_pre_dispatch;
		assert!(
			evm_fee > native_fee,
			"assertion failed evm_fee > native fee. Evm fee: {:?} Native fee: {:?}",
			evm_fee,
			native_fee
		);

		let fee_difference = evm_fee - native_fee;
		assert!(fee_difference > 0);

		let relative_fee_difference = FixedU128::from_rational(fee_difference, native_fee);
		let tolerated_fee_difference = FixedU128::from_rational(35, 100);
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
fn substrate_account_should_pay_gas_with_payment_currency() {
	TestNet::reset();
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		// Arrange
		let evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())));
		assert_eq!(EVMAccounts::bound_account_id(evm_address), Some(ALICE.into()));
		assert_eq!(
			hydradx_runtime::MultiTransactionPayment::account_currency(&AccountId::from(ALICE)),
			0
		);
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			WETH,
			to_ether(1),
			0,
		));
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			HDX,
			100_000_000_000_000,
		));

		let initial_alice_hdx_balance = Currencies::free_balance(HDX, &AccountId::from(ALICE));

		// Act
		assert_ok!(EVM::call(
			RuntimeOrigin::signed(ALICE.into()),
			evm_address,
			hydradx_runtime::evm::precompiles::IDENTITY,
			vec![],
			U256::zero(),
			1000000,
			U256::from(1000000000),
			None,
			Some(U256::zero()),
			[].into()
		));

		// Assert
		assert_eq!(
			Tokens::free_balance(WETH, &AccountId::from(ALICE)),
			to_ether(1),
			"ether balance shouldn't be touched"
		);

		let alice_hdx_balance = Currencies::free_balance(HDX, &AccountId::from(ALICE));
		let diff = initial_alice_hdx_balance - alice_hdx_balance;
		assert!(diff > 0);
	});
}

#[test]
fn evm_account_pays_with_weth_for_evm_call_if_payment_currency_not_set() {
	TestNet::reset();
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		// Arrange
		let evm_address = EVMAccounts::evm_address(&evm_account());
		assert!(EVMAccounts::is_evm_account(evm_account()));
		assert_eq!(
			hydradx_runtime::MultiTransactionPayment::account_currency(&evm_account()),
			WETH
		);
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			evm_account(),
			WETH,
			to_ether(1),
			0,
		));
		assert_ok!(Currencies::update_balance(RuntimeOrigin::root(), evm_account(), HDX, 0,));
		let mut padded_evm_address = [0u8; 32];
		padded_evm_address[..20].copy_from_slice(evm_address.as_bytes());

		// Act
		assert_ok!(EVM::call(
			RuntimeOrigin::signed(padded_evm_address.into()),
			evm_address,
			hydradx_runtime::evm::precompiles::IDENTITY,
			vec![],
			U256::zero(),
			1000000,
			U256::from(1000000000),
			None,
			Some(U256::zero()),
			[].into()
		));

		// Assert
		assert_ne!(
			Tokens::free_balance(WETH, &evm_account()),
			to_ether(1),
			"ether balance should be touched"
		);
	});
}

#[test]
fn evm_account_should_pay_gas_with_payment_currency_for_evm_call() {
	TestNet::reset();
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		// Arrange
		assert!(EVMAccounts::is_evm_account(evm_account()));
		assert_eq!(
			hydradx_runtime::MultiTransactionPayment::account_currency(&evm_account()),
			WETH
		);

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			evm_account(),
			WETH,
			to_ether(1),
			0,
		));
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			evm_account(),
			HDX,
			1000 * UNITS as i128,
		));

		set_evm_account_currency(HDX);

		assert_eq!(
			hydradx_runtime::MultiTransactionPayment::account_currency(&evm_account()),
			HDX
		);

		let initial_hdx_balance = Currencies::free_balance(HDX, &evm_account());

		// Act
		assert_ok!(EVM::call(
			evm_signed_origin(evm_address()),
			evm_address(),
			hydradx_runtime::evm::precompiles::IDENTITY,
			vec![],
			U256::zero(),
			1000000,
			U256::from(1000000000),
			None,
			Some(U256::zero()),
			[].into()
		));

		let hdx_balance = Currencies::free_balance(HDX, &evm_account());

		// Assert
		assert_eq!(
			Tokens::free_balance(WETH, &evm_account()),
			to_ether(1),
			"ether balance shouldn't be touched"
		);

		assert_ne!(initial_hdx_balance, hdx_balance, "payment asset should be touched");
	});
}

pub fn init_omnipool_with_oracle_for_block_10() {
	init_omnipol();
	hydradx_run_to_next_block();
	do_trade_to_populate_oracle(WETH, DOT, 1_000_000_000_000);
	let to = 20;
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
	let acc = Omnipool::protocol_account();

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
	assert_ok!(Omnipool::add_token(
		RuntimeOrigin::root(),
		HDX,
		native_price,
		Permill::from_percent(60),
		AccountId::from(ALICE),
	));

	assert_ok!(Omnipool::add_token(
		RuntimeOrigin::root(),
		DOT,
		dot_price,
		Permill::from_percent(60),
		AccountId::from(ALICE),
	));
	assert_ok!(Omnipool::add_token(
		RuntimeOrigin::root(),
		WETH,
		weth_price,
		Permill::from_percent(60),
		AccountId::from(ALICE),
	));

	assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), acc, DAI, stable_amount, 0));
	assert_ok!(Omnipool::add_token(
		RuntimeOrigin::root(),
		DAI,
		stable_price,
		Permill::from_percent(100),
		AccountId::from(ALICE),
	));

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		Treasury::account_id(),
		TREASURY_ACCOUNT_INIT_BALANCE,
	));
}

// TODO: test that we charge approximatelly same fee on evm as with extrinsics directly
pub fn gas_price() -> U256 {
	U256::from(hydradx_runtime::evm::DEFAULT_BASE_FEE_PER_GAS)
}

impl MockHandle {
	pub fn new_dispatch(sender: H160, data: Vec<u8>) -> Self {
		Self {
			input: data,
			context: Context {
				address: DISPATCH_ADDR,
				caller: sender,
				apparent_value: U256::zero(),
			},
			code_address: DISPATCH_ADDR,
			is_static: true,
		}
	}
}

pub fn create_dispatch_handle(data: Vec<u8>) -> MockHandle {
	MockHandle::new_dispatch(evm_address(), data)
}

pub fn native_asset_ethereum_address() -> H160 {
	H160::from(hex!("0000000000000000000000000000000100000000"))
}

pub fn dai_ethereum_address() -> H160 {
	H160::from(hex!("0000000000000000000000000000000100000002"))
}

pub fn set_evm_account_currency(currency: AssetId) {
	let set_currency_call =
		RuntimeCall::MultiTransactionPayment(pallet_transaction_multi_payment::Call::set_currency { currency })
			.encode();

	let mut handle = create_dispatch_handle(set_currency_call);
	let precompiles = HydraDXPrecompiles::<hydradx_runtime::Runtime>::new();
	let result = precompiles.execute(&mut handle).unwrap();
	assert_eq!(
		result,
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Stopped,
			output: Default::default()
		})
	);
}

pub struct MockHandle {
	pub input: Vec<u8>,
	pub context: Context,
	pub code_address: H160,
	pub is_static: bool,
}

impl PrecompileHandle for MockHandle {
	fn call(
		&mut self,
		_: H160,
		_: Option<Transfer>,
		_: Vec<u8>,
		_: Option<u64>,
		_: bool,
		_: &Context,
	) -> (ExitReason, Vec<u8>) {
		unimplemented!()
	}

	fn record_cost(&mut self, _: u64) -> Result<(), ExitError> {
		Ok(())
	}

	fn record_external_cost(
		&mut self,
		_ref_time: Option<u64>,
		_proof_size: Option<u64>,
		_storage_growth: Option<u64>,
	) -> Result<(), ExitError> {
		Ok(())
	}

	fn refund_external_cost(&mut self, _ref_time: Option<u64>, _proof_size: Option<u64>) {}

	fn remaining_gas(&self) -> u64 {
		unimplemented!()
	}

	fn log(&mut self, _: H160, _: Vec<H256>, _: Vec<u8>) -> Result<(), ExitError> {
		unimplemented!()
	}

	fn code_address(&self) -> H160 {
		self.code_address
	}

	fn input(&self) -> &[u8] {
		&self.input
	}

	fn context(&self) -> &Context {
		&self.context
	}

	fn origin(&self) -> H160 {
		todo!()
	}

	fn is_static(&self) -> bool {
		self.is_static
	}

	fn gas_limit(&self) -> Option<u64> {
		None
	}

	fn is_contract_being_constructed(&self, _address: H160) -> bool {
		todo!()
	}
}

fn create_xyk_pool_with_amounts(asset_a: u32, amount_a: u128, asset_b: u32, amount_b: u128) {
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		DAVE.into(),
		asset_a,
		amount_a as i128,
	));
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		DAVE.into(),
		asset_b,
		amount_b as i128,
	));

	assert_ok!(XYK::create_pool(
		RuntimeOrigin::signed(DAVE.into()),
		asset_a,
		amount_a,
		asset_b,
		amount_b,
	));
}

mod evm_error_decoder {
	use super::*;
	use codec::{Decode, DecodeLimit};
	use hydradx_runtime::evm::evm_error_decoder::EvmErrorDecoder;
	use hydradx_runtime::evm::evm_error_decoder::*;
	use hydradx_traits::evm::CallResult;
	use pallet_evm::{ExitError, ExitFatal, ExitReason, ExitRevert, ExitSucceed};
	use proptest::prelude::*;
	use proptest::test_runner::{Config, TestRunner};
	use sp_core::Get;
	use sp_runtime::traits::Convert;
	use sp_runtime::{DispatchError, DispatchResult};

	fn arbitrary_value() -> impl Strategy<Value = Vec<u8>> {
		prop::collection::vec(any::<u8>(), 0..256)
	}

	fn random_error_string() -> impl Strategy<Value = Vec<u8>> {
		// Fixed 4-byte prefix for Error(string) solidity error
		let prefix: [u8; 4] = [0x08, 0xC3, 0x79, 0xA0];

		// Generate the remaining random bytes (0–252)
		prop::collection::vec(any::<u8>(), 0..252).prop_map(move |mut rest| {
			let mut bytes = Vec::with_capacity(4 + rest.len());
			bytes.extend_from_slice(&prefix);
			bytes.append(&mut rest);
			bytes
		})
	}

	fn arbitrary_contract() -> impl Strategy<Value = sp_core::H160> {
		prop::array::uniform20(any::<u8>()).prop_map(H160::from)
	}

	fn arbitrary_exit_reason() -> impl Strategy<Value = ExitReason> {
		prop_oneof![
			Just(ExitReason::Succeed(ExitSucceed::Stopped)),
			Just(ExitReason::Succeed(ExitSucceed::Returned)),
			Just(ExitReason::Succeed(ExitSucceed::Suicided)),
			Just(ExitReason::Error(ExitError::StackUnderflow)),
			Just(ExitReason::Error(ExitError::StackOverflow)),
			Just(ExitReason::Error(ExitError::InvalidJump)),
			Just(ExitReason::Revert(ExitRevert::Reverted)),
			Just(ExitReason::Fatal(ExitFatal::NotSupported)),
		]
	}

	/// Property-based test to ensure EvmErrorDecoder never panics
	/// with arbitrary input values, exit reasons, and contract addresses
	proptest! {
		#![proptest_config(ProptestConfig::with_cases(10000))]
		#[test]
		fn evm_error_decoder_never_panics(
			value in arbitrary_value(),
			exit_reason in arbitrary_exit_reason(),
			contract in arbitrary_contract(),
		) {
			let call_result = CallResult {
					exit_reason,
					value,
					contract,
				};

			let _result = EvmErrorDecoder::convert(call_result);
		}
	}

	//We set up prop test like this to share state, so we don't need to load snapshot in every run
	#[test]
	fn evm_error_decoder_never_panics_for_borrowing_contract() {
		let successfull_cases = 10000;

		hydra_live_ext(crate::liquidation::PATH_TO_SNAPSHOT).execute_with(|| {
			// We run prop test this way to use the same state of the chain for all run without loading the snapshot again in every run
			let mut runner = TestRunner::new(Config {
				cases: successfull_cases,
				source_file: Some("integration-tests/src/evm.rs"),
				test_name: Some("evm_prop"),
				..Config::default()
			});

			let _ = runner
				.run(&random_error_string(), |value| {
					let call_result = CallResult {
						exit_reason: ExitReason::Error(ExitError::Other("Some error".into())),
						value,
						contract: hydradx_runtime::Liquidation::get(),
					};

					let _result = EvmErrorDecoder::convert(call_result);

					Ok(())
				})
				.unwrap();
		});
	}

	#[test]
	fn evm_error_decoder_handles_empty_value() {
		let call_result = CallResult {
			exit_reason: ExitReason::Revert(ExitRevert::Reverted),
			value: vec![],
			contract: sp_core::H160::zero(),
		};

		let _error = EvmErrorDecoder::convert(call_result);
	}

	#[test]
	fn evm_error_decoder_handles_values_shorter_than_function_selector_length() {
		let call_result = CallResult {
			exit_reason: ExitReason::Revert(ExitRevert::Reverted),
			value: vec![0x01, 0x02],
			contract: sp_core::H160::zero(),
		};

		let _error = EvmErrorDecoder::convert(call_result);
	}

	#[test]
	fn decode_should_not_panic_on_deeply_nested_input() {
		// Test 1: Deeply nested payload (simulating stack exhaustion attack)
		let mut nested_payload = vec![0x01];
		for _ in 0..10000 {
			let mut new_layer = vec![0x01];
			new_layer.extend_from_slice(&nested_payload);
			nested_payload = new_layer;
		}

		let result = DispatchError::decode(&mut &nested_payload[..]).unwrap();

		pretty_assertions::assert_eq!(result, DispatchError::CannotLookup);
	}

	#[test]
	fn value_with_max_length_no_truncation_should_not_panic() {
		let value = vec![0xFF; MAX_ERROR_DATA_LENGTH];

		let call_result = CallResult {
			exit_reason: ExitReason::Revert(ExitRevert::Reverted),
			value: value.clone(),
			contract: sp_core::H160::zero(),
		};

		let result = EvmErrorDecoder::convert(call_result);

		assert!(matches!(result, DispatchError::Other(_)));
	}

	#[test]
	fn value_with_max_length_plus_one_should_not_panic() {
		let value = vec![0xFF; MAX_ERROR_DATA_LENGTH + 1]; // 1025 bytes

		let call_result = CallResult {
			exit_reason: ExitReason::Revert(ExitRevert::Reverted),
			value: value.clone(),
			contract: sp_core::H160::zero(),
		};

		let result = EvmErrorDecoder::convert(call_result);

		assert!(matches!(result, DispatchError::Other(_)));
	}

	#[test]
	fn value_with_large_length_should_not_panic() {
		let value = vec![0xAB; 2048]; // 2048 bytes

		let call_result = CallResult {
			exit_reason: ExitReason::Revert(ExitRevert::Reverted),
			value: value.clone(),
			contract: sp_core::H160::zero(),
		};

		let result = EvmErrorDecoder::convert(call_result);

		assert!(matches!(result, DispatchError::Other(_)));
	}

	#[test]
	fn value_with_lenth_minus_one_should_not_panic() {
		let value = vec![0x42; MAX_ERROR_DATA_LENGTH - 1];

		let call_result = CallResult {
			exit_reason: ExitReason::Revert(ExitRevert::Reverted),
			value: value.clone(),
			contract: sp_core::H160::zero(),
		};

		let result = EvmErrorDecoder::convert(call_result);

		assert!(matches!(result, DispatchError::Other(_)));
	}

	#[test]
	fn value_with_extremely_large_length_should_not_panic() {
		let value = vec![0xFF; 10_000]; // 10KB

		let call_result = CallResult {
			exit_reason: ExitReason::Revert(ExitRevert::Reverted),
			value: value.clone(),
			contract: sp_core::H160::zero(),
		};

		let result = EvmErrorDecoder::convert(call_result);

		assert!(matches!(result, DispatchError::Other(_)));
	}

	#[test]
	fn test_off_by_one_boundary() {
		// Test the exact boundary condition
		let sizes = vec![
			MAX_ERROR_DATA_LENGTH - 2,
			MAX_ERROR_DATA_LENGTH - 1,
			MAX_ERROR_DATA_LENGTH,
			MAX_ERROR_DATA_LENGTH + 1,
			MAX_ERROR_DATA_LENGTH + 2,
		];

		for size in sizes {
			let value = vec![0xAA; size];
			let call_result = CallResult {
				exit_reason: ExitReason::Revert(ExitRevert::Reverted),
				value,
				contract: sp_core::H160::zero(),
			};

			let result = EvmErrorDecoder::convert(call_result);

			assert!(matches!(result, DispatchError::Other(_)));
		}
	}

	#[test]
	fn test_scale_decode_invalid_discriminant() {
		// DispatchError enum has valid discriminants 0-12
		// Let's try an invalid discriminant like 0xFF
		let value = vec![0xFF, 0x00, 0x00, 0x00];

		let call_result = CallResult {
			exit_reason: ExitReason::Revert(ExitRevert::Reverted),
			value,
			contract: sp_core::H160::zero(),
		};

		let result = EvmErrorDecoder::convert(call_result);

		assert!(matches!(result, DispatchError::Other(_)));
	}

	#[test]
	fn test_scale_decode_various_invalid_discriminants() {
		let invalid_discriminants = vec![0xFFu8, 0xFE, 0xFD, 0x80, 0x7F, 0x20, 0x15];

		for discriminant in invalid_discriminants {
			let value = vec![discriminant, 0x00, 0x00, 0x00];
			let call_result = CallResult {
				exit_reason: ExitReason::Revert(ExitRevert::Reverted),
				value,
				contract: sp_core::H160::zero(),
			};

			let result = EvmErrorDecoder::convert(call_result);
			assert!(matches!(result, DispatchError::Other(_)));
		}
	}

	#[test]
	fn test_scale_decode_malformed_compact_length() {
		// Compact encoding with invalid length prefix
		// Format: [discriminant, compact_length_bytes..., data...]
		let malformed_values = vec![
			// Length prefix indicates huge size but no data follows
			vec![0x00, 0xFF, 0xFF, 0xFF, 0xFF],
			// Incomplete compact length
			vec![0x00, 0xFD],
			// Length overflow scenario
			vec![0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
		];

		for value in malformed_values {
			let call_result = CallResult {
				exit_reason: ExitReason::Revert(ExitRevert::Reverted),
				value,
				contract: sp_core::H160::zero(),
			};

			let result = EvmErrorDecoder::convert(call_result);
			assert!(matches!(result, DispatchError::Other(_)));
		}
	}

	#[test]
	fn test_scale_decode_truncated_data() {
		let truncated_values = vec![
			vec![0x00],             // Just discriminant, no data
			vec![0x03],             // Module error discriminant but no module data
			vec![0x03, 0x00],       // Module error with incomplete data
			vec![0x03, 0x00, 0x00], // Module error with more incomplete data
		];

		for value in truncated_values {
			let call_result = CallResult {
				exit_reason: ExitReason::Revert(ExitRevert::Reverted),
				value,
				contract: sp_core::H160::zero(),
			};

			let result = EvmErrorDecoder::convert(call_result);
			assert!(matches!(result, DispatchError::Other(_)));
		}
	}

	#[test]
	fn test_scale_decode_nested_structure() {
		// Try to create deeply nested structure that might exceed depth limit
		// This simulates a malicious payload trying to exhaust the stack
		let mut nested_data = vec![0x00u8]; // Start with valid discriminant

		// Add many layers of nesting indicators
		for _ in 0..300 {
			nested_data.push(0x01); // Indicate nested structure
		}

		let call_result = CallResult {
			exit_reason: ExitReason::Revert(ExitRevert::Reverted),
			value: nested_data,
			contract: sp_core::H160::zero(),
		};

		let result = EvmErrorDecoder::convert(call_result);

		assert!(matches!(result, DispatchError::Other(_)));
	}

	#[test]
	fn test_scale_decode_length_overflow() {
		// Try to trigger integer overflow in length calculation
		// Use maximum values for length fields
		let overflow_values = vec![
			// Max u32 as compact length
			vec![0x00, 0x03, 0xFF, 0xFF, 0xFF, 0xFF],
			// Max u64 representation in compact encoding
			vec![0x00, 0x13, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
			// Multiple max values
			vec![0xFF; 32],
		];

		for value in overflow_values {
			let call_result = CallResult {
				exit_reason: ExitReason::Revert(ExitRevert::Reverted),
				value,
				contract: sp_core::H160::zero(),
			};

			let result = EvmErrorDecoder::convert(call_result);
			assert!(matches!(result, DispatchError::Other(_)));
		}
	}

	#[test]
	fn test_scale_decode_random_garbage() {
		let garbage_values = vec![
			vec![0xDE, 0xAD, 0xBE, 0xEF],
			vec![0xFF; 100],
			vec![0x00; 100],
			vec![0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55],
		];

		for value in garbage_values {
			let call_result = CallResult {
				exit_reason: ExitReason::Revert(ExitRevert::Reverted),
				value,
				contract: sp_core::H160::zero(),
			};

			let result = EvmErrorDecoder::convert(call_result);
			assert!(matches!(result, DispatchError::Other(_)));
		}
	}

	#[test]
	fn test_scale_decode_empty_data() {
		// Empty vector should fail to decode but not panic
		let value = vec![];

		let call_result = CallResult {
			exit_reason: ExitReason::Revert(ExitRevert::Reverted),
			value,
			contract: sp_core::H160::zero(),
		};

		let result = EvmErrorDecoder::convert(call_result);

		assert!(matches!(result, DispatchError::Other(_)));
	}

	#[test]
	fn test_scale_decode_all_single_bytes_discriminants() {
		// Test every possible discriminant value
		for byte in 0u8..=255 {
			let value = vec![byte];
			let call_result = CallResult {
				exit_reason: ExitReason::Revert(ExitRevert::Reverted),
				value,
				contract: sp_core::H160::zero(),
			};

			let result = EvmErrorDecoder::convert(call_result);

			assert!(matches!(result, DispatchError::Other(_) | DispatchError::CannotLookup));
		}
	}

	#[test]
	fn test_scale_decode_malicious_payload() {
		let malicious_payloads = vec![
			// Looks like Module error (discriminant 3) with crafted data
			vec![0x03, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00],
			// Looks like Other variant (discriminant 0) with invalid string data
			vec![0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
			// Token error with invalid nested enum
			vec![0x06, 0xFF, 0xFF, 0xFF, 0xFF],
			// Arithmetic error with invalid nested enum
			vec![0x07, 0xFF, 0xFF, 0xFF, 0xFF],
		];

		for value in malicious_payloads.iter() {
			let call_result = CallResult {
				exit_reason: ExitReason::Revert(ExitRevert::Reverted),
				value: value.clone(),
				contract: sp_core::H160::zero(),
			};

			let _result = EvmErrorDecoder::convert(call_result);
		}
	}

	#[test]
	fn dispatch_decode_with_malformed_scawle_payloads_should_not_panic() {
		// Test various malicious/malformed SCALE-encoded payloads
		// that could trigger panics in decode_with_depth_limit
		let test_cases = vec![
			("Empty vector", vec![]),
			("Single invalid discriminant (255)", vec![0xFF]),
			("Invalid discriminant with data", vec![0xFF, 0x00, 0x00, 0x00]),
			("Deeply nested (10000 bytes of 0x01)", vec![0x01; 10000]),
			("Truncated Module error", vec![0x03, 0x00]),
			("Invalid compact length", vec![0x00, 0xFF, 0xFF, 0xFF, 0xFF]),
			("All zeros", vec![0x00; 100]),
			("All ones", vec![0xFF; 100]),
			("Random garbage", vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE]),
			("Length overflow", vec![0x00, 0x03, 0xFF, 0xFF, 0xFF, 0xFF]),
			(
				"Huge compact (u64 max)",
				vec![0x00, 0x13, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
			),
			("Malformed Module error 1", vec![0x03]),
			("Malformed Module error 2", vec![0x03, 0xFF, 0xFF]),
			("Malformed Token error", vec![0x06, 0xFF]),
			("Malformed Arithmetic error", vec![0x07, 0xFF]),
			// Additional edge cases for SCALE decoding
			("Nested depth attack", vec![0x01; 500]),
			("Large compact prefix", vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
			("Module error with overflow", vec![0x03, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
		];

		for (_name, value) in test_cases {
			let call_result = CallResult {
				exit_reason: ExitReason::Revert(ExitRevert::Reverted),
				value,
				contract: sp_core::H160::zero(),
			};

			let _result = EvmErrorDecoder::convert(call_result.clone());
			DispatchError::decode_with_depth_limit(MAX_DECODE_DEPTH, &mut &call_result.value[..]);
		}
	}

	#[test]
	fn dispatch_decode_cannot_pani_for_different_multi_byte_patterns() {
		for byte1 in [0x00, 0x03, 0x06, 0x07, 0xFF].iter() {
			for byte2 in [0x00, 0xFF].iter() {
				let call_result = CallResult {
					exit_reason: ExitReason::Revert(ExitRevert::Reverted),
					value: vec![*byte1, *byte2],
					contract: sp_core::H160::zero(),
				};

				let _result = EvmErrorDecoder::convert(call_result.clone());

				let _ = DispatchError::decode_with_depth_limit(MAX_DECODE_DEPTH, &mut &call_result.value[..]);
			}
		}
	}

	#[test]
	fn test_aave_error_with_exact_70_bytes_length() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let error_data = create_aave_error_with_exact_length(b"35");
			pretty_assertions::assert_eq!(error_data.len(), 70, "Error data must be exactly 70 bytes");

			let call_result = CallResult {
				exit_reason: ExitReason::Succeed(ExitSucceed::Returned),
				value: error_data,
				contract: hydradx_runtime::Liquidation::get(),
			};

			let result = EvmErrorDecoder::convert(call_result);

			pretty_assertions::assert_eq!(
				result,
				pallet_dispatcher::Error::<hydradx_runtime::Runtime>::AaveHealthFactorLowerThanLiquidationThreshold
					.into()
			);
		});
	}

	#[test]
	fn test_non_aave_contract_with_70_bytes_falls_back_to_generic() {
		TestNet::reset();

		Hydra::execute_with(|| {
			// With non-AAVE contract address, should fall back to generic error
			let error_data = create_aave_error_with_exact_length(b"35");

			let call_result = CallResult {
				exit_reason: ExitReason::Succeed(ExitSucceed::Returned),
				value: error_data,
				contract: H160::from_low_u64_be(12345), // Different contract
			};

			let result = EvmErrorDecoder::convert(call_result);

			assert!(matches!(result, DispatchError::Other(_)));
		});
	}

	fn create_aave_error_with_exact_length(error_code: &[u8; 2]) -> Vec<u8> {
		let mut error_data = vec![0u8; 70];

		// Set Error(string) selector [0x08, 0xC3, 0x79, 0xA0]
		error_data[0..4].copy_from_slice(&[0x08, 0xC3, 0x79, 0xA0]);

		// Bytes 4-65: padding (62 bytes of zeros is fine for testing)

		// Bytes 66-67: Error string length marker [0x00, 0x02]
		error_data[66] = 0x00;
		error_data[67] = 0x02;

		// Bytes 68-69: Error code (e.g., b"35")
		error_data[68] = error_code[0];
		error_data[69] = error_code[1];

		assert!(error_data.len() == 70, "Error data must be exactly 70 bytes");

		error_data
	}
}
