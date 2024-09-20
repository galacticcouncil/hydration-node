use crate::evm::native_asset_ethereum_address;
use crate::polkadot_test_net::Hydra;
use crate::polkadot_test_net::TestNet;
use crate::polkadot_test_net::ALICE;
use crate::utils::contracts::deploy_contract;
use hex_literal::hex;
use hydradx_runtime::evm::Executor;
use hydradx_runtime::AccountId;
use hydradx_runtime::EVMAccounts;
use hydradx_runtime::Runtime;
use hydradx_traits::evm::CallContext;
use hydradx_traits::evm::EvmAddress;
use hydradx_traits::evm::InspectEvmAccounts;
use hydradx_traits::evm::EVM;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_evm::ExitReason::Succeed;
use sp_core::H256;
use sp_core::{RuntimeDebug, U256};
use xcm_emulator::Network;
use xcm_emulator::TestExt;

pub fn deployer() -> EvmAddress {
	EVMAccounts::evm_address(&Into::<AccountId>::into(ALICE))
}

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	IsContract = "isContract(address)",
	Check = "check(address)",
}

fn is_contract(checker: EvmAddress, address: EvmAddress) -> bool {
	let mut data = Into::<u32>::into(Function::Check).to_be_bytes().to_vec();
	data.extend_from_slice(H256::from(address).as_bytes());
	let context = CallContext {
		contract: checker,
		sender: Default::default(),
		origin: Default::default(),
	};
	let (res, _) = Executor::<Runtime>::call(context, data, U256::zero(), 100_000);
	match res {
		Succeed(_) => true,
		_ => false,
	}
}

#[test]
fn contract_check_succeeds_on_deployed_contract() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let checker = deploy_contract("ContractCheck", deployer());

		assert_eq!(is_contract(checker, checker), true);
	});
}

#[test]
fn contract_check_fails_on_eoa() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let checker = deploy_contract("ContractCheck", deployer());

		assert_eq!(is_contract(checker, deployer()), false);
	});
}

#[test]
fn contract_check_fails_on_precompile() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let checker = deploy_contract("ContractCheck", deployer());

		assert_eq!(is_contract(checker, native_asset_ethereum_address()), false);
	});
}

#[test]
fn contract_check_succeeds_on_precompile_with_code() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let checker = deploy_contract("ContractCheck", deployer());
		pallet_evm::AccountCodes::<Runtime>::insert(
			native_asset_ethereum_address(),
			&hex!["365f5f375f5f365f73bebebebebebebebebebebebebebebebebebebebe5af43d5f5f3e5f3d91602a57fd5bf3"][..],
		);
		assert_eq!(is_contract(checker, native_asset_ethereum_address()), true);
	});
}

#[test]
fn contract_check_succeeds_on_precompile_with_invalid_code() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let checker = deploy_contract("ContractCheck", deployer());
		pallet_evm::AccountCodes::<Runtime>::insert(native_asset_ethereum_address(), &hex!["00"][..]);
		assert_eq!(is_contract(checker, native_asset_ethereum_address()), true);
	});
}
