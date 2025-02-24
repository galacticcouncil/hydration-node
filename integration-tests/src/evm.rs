#![cfg(test)]

use crate::{assert_balance, polkadot_test_net::*};
use fp_evm::{Context, Transfer};
use fp_rpc::runtime_decl_for_ethereum_runtime_rpc_api::EthereumRuntimeRPCApi;
use frame_support::storage::with_transaction;
use frame_support::traits::fungible::Mutate;
use frame_support::{assert_ok, dispatch::GetDispatchInfo, sp_runtime::codec::Encode, traits::Contains};
use frame_system::RawOrigin;
use hex_literal::hex;
use hydradx_runtime::evm::precompiles::DISPATCH_ADDR;
use hydradx_runtime::evm::EvmAddress;
use hydradx_runtime::evm::ExtendedAddressMapping;
use hydradx_runtime::evm::Function;
use hydradx_runtime::XYK;
use hydradx_runtime::{
	evm::precompiles::{
		handle::EvmDataWriter, multicurrency::MultiCurrencyPrecompile, Address, Bytes, HydraDXPrecompiles,
	},
	AssetRegistry, Balances, CallFilter, Currencies, EVMAccounts, Omnipool, RuntimeCall, RuntimeOrigin, Tokens,
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
use sp_runtime::TransactionOutcome;
use sp_runtime::{traits::SignedExtension, DispatchError, FixedU128, Permill};
use std::borrow::Cow;
use xcm_emulator::TestExt;

pub const TREASURY_ACCOUNT_INIT_BALANCE: Balance = 1000 * UNITS;

mod account_conversion {
	use super::*;
	use fp_evm::ExitSucceed;
	use frame_support::{assert_noop, assert_ok};
	use pretty_assertions::assert_eq;

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

			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
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
			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				ALICE.into()
			)),);

			assert_noop!(
				EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())),
				pallet_evm_accounts::Error::<hydradx_runtime::Runtime>::AddressAlreadyBound,
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

			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
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
				EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())),
				pallet_evm_accounts::Error::<hydradx_runtime::Runtime>::TruncatedAccountAlreadyUsed,
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

			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
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
			let result = MultiCurrencyPrecompile::<hydradx_runtime::Runtime>::execute(&mut handle);

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
			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				ALICE.into()
			)),);

			let result = MultiCurrencyPrecompile::<hydradx_runtime::Runtime>::execute(&mut handle);

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
			let call = pallet_evm_accounts::Call::<hydradx_runtime::Runtime>::bind_evm_address {};
			let info = call.get_dispatch_info();
			// convert to outer call
			let call = hydradx_runtime::RuntimeCall::EVMAccounts(call);
			let len = call.using_encoded(|e| e.len()) as u32;

			NextFeeMultiplier::<hydradx_runtime::Runtime>::put(Multiplier::saturating_from_integer(1));
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
				U256::from(53000u64),
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

			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				ALICE.into()
			)),);

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
				pallet_evm_accounts::Error::<hydradx_runtime::Runtime>::BoundAddressCannotBeUsed
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

			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				ALICE.into()
			)),);

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
}

mod standard_precompiles {
	use super::*;
	use frame_support::assert_ok;
	use pretty_assertions::assert_eq;
	use sp_runtime::traits::UniqueSaturatedInto;

	fn evm_runner_call(
		to: EvmAddress,
		data: Vec<u8>,
	) -> Result<CallInfo, RunnerError<pallet_evm::Error<hydradx_runtime::Runtime>>> {
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			evm_account(),
			WETH,
			to_ether(1_000),
			0,
		));
		<hydradx_runtime::Runtime as pallet_evm::Config>::Runner::call(
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
			<hydradx_runtime::Runtime as pallet_evm::Config>::config(),
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
				hydradx_runtime::RuntimeOrigin::root(),
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
				hydradx_runtime::RuntimeOrigin::root(),
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
			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
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
			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
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
			pallet_evm::AccountCodes::<hydradx_runtime::Runtime>::insert(
				native_asset_ethereum_address(),
				&hex!["365f5f375f5f365f73bebebebebebebebebebebebebebebebebebebebe5af43d5f5f3e5f3d91602a57fd5bf3"][..],
			);

			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
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
			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
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
					exit_status: pallet_evm::ExitError::Other("not supported".into())
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
			assert_ok!(EVMAccounts::approve_contract(
				hydradx_runtime::RuntimeOrigin::root(),
				evm_address(),
			));

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
			assert_ok!(EVMAccounts::approve_contract(
				hydradx_runtime::RuntimeOrigin::root(),
				evm_address(),
			));
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
			assert_ok!(EVMAccounts::disapprove_contract(
				hydradx_runtime::RuntimeOrigin::root(),
				evm_address(),
			));
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
			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
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
				hydradx_runtime::RuntimeOrigin::root(),
				native_asset_ethereum_address(),
			));

			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
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
	use ethabi::ethereum_types::U256;
	use frame_support::{
		sp_runtime::{FixedPointNumber, FixedU128},
		traits::ConstU32,
	};
	use hydradx_runtime::{
		evm::precompiles::chainlink_adapter::{encode_evm_address, AggregatorInterface, ChainlinkOraclePrecompile},
		EmaOracle, Router,
	};
	use hydradx_traits::{router::AssetPair, AggregatedPriceOracle, OraclePeriod};
	use pallet_ema_oracle::Price;
	use primitives::constants::chain::{OMNIPOOL_SOURCE, XYK_SOURCE};

	fn assert_prices_are_same(ema_price: Price, precompile_price: U256, decimals: u8) {
		let fixed_price_int = FixedU128::checked_from_rational(ema_price.n, ema_price.d)
			.unwrap()
			.checked_mul_int(10_u128.pow(decimals.into()))
			.unwrap();

		pretty_assertions::assert_eq!(fixed_price_int, precompile_price.as_u128());
	}

	#[test]
	fn chainlink_precompile_should_work_with_omnipool_source() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			hydradx_run_to_next_block();

			init_omnipool();

			let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				token_price,
				Permill::from_percent(100),
				AccountId::from(BOB),
			));

			assert_ok!(hydradx_runtime::Omnipool::sell(
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

			let oracle_ethereum_address = encode_evm_address(HDX, OraclePeriod::Short, OMNIPOOL_SOURCE).unwrap();

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
				ChainlinkOraclePrecompile::<ConstU32<DOT>, hydradx_runtime::Runtime>::execute(&mut handle).unwrap();

			//Assert
			pretty_assertions::assert_eq!(exit_status, ExitSucceed::Returned,);

			assert_prices_are_same(ema_price, U256::from_big_endian(&output), 12);
		});
	}

	#[test]
	fn chainlink_precompile_should_work_with_xyk_source() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			hydradx_run_to_next_block();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				DOT,
				200 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::XYK::create_pool(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				100 * UNITS,
				DOT,
				200 * UNITS,
			));

			assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), XYK_SOURCE, (HDX, DOT)));

			assert_ok!(hydradx_runtime::XYK::buy(
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

			let oracle_ethereum_address = encode_evm_address(HDX, OraclePeriod::Short, XYK_SOURCE).unwrap();

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
				ChainlinkOraclePrecompile::<ConstU32<DOT>, hydradx_runtime::Runtime>::execute(&mut handle).unwrap();

			//Assert
			pretty_assertions::assert_eq!(exit_status, ExitSucceed::Returned,);

			assert_prices_are_same(ema_price, U256::from_big_endian(&output), 12);
		});
	}

	#[test]
	fn chainlink_precompile_should_work_with_routed_pair() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			hydradx_run_to_next_block();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				DOT,
				200 * UNITS as i128,
			));

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				DAI,
				200 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::XYK::create_pool(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				100 * UNITS,
				DAI,
				200 * UNITS,
			));

			assert_ok!(hydradx_runtime::XYK::create_pool(
				RuntimeOrigin::signed(ALICE.into()),
				DAI,
				100 * UNITS,
				DOT,
				300 * UNITS,
			));

			assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), XYK_SOURCE, (HDX, DAI)));
			assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), XYK_SOURCE, (DAI, DOT)));

			assert_ok!(hydradx_runtime::XYK::buy(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				2 * UNITS,
				200 * UNITS,
				false,
			));
			assert_ok!(hydradx_runtime::XYK::buy(
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
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				AssetPair::new(HDX, DOT),
				route
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

			let oracle_ethereum_address = encode_evm_address(HDX, OraclePeriod::Short, [0; 8]).unwrap();

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
				ChainlinkOraclePrecompile::<ConstU32<DOT>, hydradx_runtime::Runtime>::execute(&mut handle).unwrap();

			//Assert
			pretty_assertions::assert_eq!(exit_status, ExitSucceed::Returned,);

			assert_prices_are_same(ema_price, U256::from_big_endian(&output), 12);
		});
	}

	#[test]
	fn chainlink_precompile_should_return_error_when_oracle_not_available() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			assert!(EmaOracle::get_price(HDX, DOT, OraclePeriod::Short, XYK_SOURCE).is_err());

			let data = EvmDataWriter::new_with_selector(AggregatorInterface::GetAnswer).build();

			let oracle_ethereum_address = encode_evm_address(HDX, OraclePeriod::Short, XYK_SOURCE).unwrap();

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
			let result = ChainlinkOraclePrecompile::<ConstU32<DOT>, hydradx_runtime::Runtime>::execute(&mut handle);

			//Assert
			pretty_assertions::assert_eq!(
				result,
				Err(PrecompileFailure::Error {
					exit_status: ExitError::Other("Price not available".into()),
				})
			);
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
			assert_ok!(EVMAccounts::add_contract_deployer(
				hydradx_runtime::RuntimeOrigin::root(),
				evm_address
			));

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
		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			ALICE.into()
		)));

		let evm_address = EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE));
		init_omnipool_with_oracle_for_block_10();
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			WETH,
			(100 * UNITS * 1_000_000) as i128,
		));
		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			WETH,
		));

		//Arrange
		let data = hex!["4d0045544800d1820d45118d78d091e685490c674d7596e62d1f0000000000000000140000000f0000c16ff28623"]
			.to_vec();
		let balance = Tokens::free_balance(WETH, &AccountId::from(ALICE));

		let (gas_price, _) = hydradx_runtime::DynamicEvmFee::min_gas_price();

		//Act
		assert_ok!(EVM::call(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
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
		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			ALICE.into()
		)));

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

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			currency_precompile::alice_substrate_evm_addr(),
			WETH,
			(100 * UNITS * 1_000_000) as i128,
		));
		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
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
			route: swap_route,
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

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
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
		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			ALICE.into()
		)));

		init_omnipool_with_oracle_for_block_10();

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			WETH,
			(10_000_000 * UNITS) as i128,
		));
		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			fee_currency,
		));

		// give alice evm addr some DOT to sell in omnipool
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			DOT,
			(10 * UNITS) as i128,
		));

		let treasury_currency_balance = Currencies::free_balance(fee_currency, &Treasury::account_id());
		let alice_currency_balance = Currencies::free_balance(fee_currency, &AccountId::from(ALICE));

		//Act
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: DOT,
				asset_out: HDX,
				amount: 10_000_000_000,
				min_buy_amount: 0,
			});

		let gas_limit = 1_000_000;
		let (gas_price, _) = hydradx_runtime::DynamicEvmFee::min_gas_price();

		//Execute omnipool sell via EVM
		assert_ok!(EVM::call(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
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
		let tolerated_fee_difference = FixedU128::from_rational(31, 100);
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
		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			ALICE.into()
		)));
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
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			HDX,
			100_000_000_000_000,
		));

		let initial_alice_hdx_balance = Currencies::free_balance(HDX, &AccountId::from(ALICE));

		// Act
		assert_ok!(EVM::call(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
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
fn evm_account_always_pays_with_weth_for_evm_call() {
	TestNet::reset();
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		// Arrange
		let evm_address = EVMAccounts::evm_address(&evm_account());
		assert!(EVMAccounts::is_evm_account(evm_account()));
		assert_eq!(
			hydradx_runtime::MultiTransactionPayment::account_currency(&evm_account()),
			0
		);
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			evm_account(),
			WETH,
			to_ether(1),
			0,
		));
		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			evm_account(),
			HDX,
			0,
		));
		let mut padded_evm_address = [0u8; 32];
		padded_evm_address[..20].copy_from_slice(evm_address.as_bytes());

		// Act
		assert_ok!(EVM::call(
			hydradx_runtime::RuntimeOrigin::signed(padded_evm_address.into()),
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

	fn is_static(&self) -> bool {
		self.is_static
	}

	fn gas_limit(&self) -> Option<u64> {
		None
	}
}

fn create_xyk_pool_with_amounts(asset_a: u32, amount_a: u128, asset_b: u32, amount_b: u128) {
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		asset_a,
		amount_a as i128,
	));
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
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
