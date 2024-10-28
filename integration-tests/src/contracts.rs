use crate::evm::dai_ethereum_address;
use crate::polkadot_test_net::Hydra;
use crate::polkadot_test_net::TestNet;
use crate::polkadot_test_net::ALICE;
use crate::polkadot_test_net::BOB;
use crate::polkadot_test_net::UNITS;
use crate::polkadot_test_net::WETH;
use crate::utils::contracts::deploy_contract;
use crate::utils::contracts::deploy_contract_code;
use crate::utils::contracts::get_contract_bytecode;
use fp_evm::ExitReason::Succeed;
use fp_evm::ExitSucceed::Stopped;
use fp_evm::FeeCalculator;
use frame_support::assert_ok;
use hex_literal::hex;
use hydradx_runtime::evm::precompiles::handle::EvmDataWriter;
use hydradx_runtime::evm::precompiles::Bytes;
use hydradx_runtime::evm::Executor;
use hydradx_runtime::AccountId;
use hydradx_runtime::EVMAccounts;
use hydradx_runtime::Runtime;
use hydradx_runtime::RuntimeEvent;
use hydradx_runtime::System;
use hydradx_traits::evm::CallContext;
use hydradx_traits::evm::EvmAddress;
use hydradx_traits::evm::InspectEvmAccounts;
use hydradx_traits::evm::EVM;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pretty_assertions::assert_eq;
use sp_core::H256;
use sp_core::{RuntimeDebug, U256};
use test_utils::expect_events;
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
	Initialize = "initialize(address)",
	InitializePayload = "initialize(address,address,bytes)",
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
	matches!(res, Succeed(_))
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
fn contract_check_succeeds_on_currencies_precompile() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let checker = deploy_contract("ContractCheck", deployer());

		assert_eq!(is_contract(checker, dai_ethereum_address()), true);
	});
}

#[test]
fn contract_check_succeeds_on_precompile_with_code() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let checker = deploy_contract("ContractCheck", deployer());
		pallet_evm::AccountCodes::<Runtime>::insert(
			dai_ethereum_address(),
			&hex!["365f5f375f5f365f73bebebebebebebebebebebebebebebebebebebebe5af43d5f5f3e5f3d91602a57fd5bf3"][..],
		);
		assert_eq!(is_contract(checker, dai_ethereum_address()), true);
	});
}

#[test]
fn contract_check_fails_on_precompile_without_code() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let checker = deploy_contract("ContractCheck", deployer());
		pallet_evm::AccountCodes::<Runtime>::remove(dai_ethereum_address());
		assert_eq!(is_contract(checker, dai_ethereum_address()), false);
	});
}

#[test]
fn contract_check_succeeds_on_precompile_with_invalid_code() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let checker = deploy_contract("ContractCheck", deployer());
		pallet_evm::AccountCodes::<Runtime>::insert(dai_ethereum_address(), &hex!["00"][..]);
		assert_eq!(is_contract(checker, dai_ethereum_address()), true);
	});
}

#[test]
fn contract_check_should_succeed_when_called_from_extrinsic() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let checker = deploy_contract("ContractCheck", deployer());
		let mut data = Into::<u32>::into(Function::Check).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(dai_ethereum_address()).as_bytes());
		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			BOB.into()
		)));
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			BOB.into(),
			WETH,
			(10_000_000 * UNITS) as i128,
		));
		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			WETH
		));
		pallet_evm::AccountCodes::<Runtime>::insert(
			dai_ethereum_address(),
			&hex!["365f5f375f5f365f73bebebebebebebebebebebebebebebebebebebebe5af43d5f5f3e5f3d91602a57fd5bf3"][..],
		);

		// Act
		assert_ok!(hydradx_runtime::EVM::call(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			EVMAccounts::evm_address(&Into::<AccountId>::into(BOB)),
			checker,
			data,
			U256::from(0),
			1000000,
			hydradx_runtime::DynamicEvmFee::min_gas_price().0 * 10,
			None,
			Some(System::account_nonce(AccountId::from(BOB)).into()),
			[].into()
		));

		// Assert
		expect_events::<RuntimeEvent, Runtime>(vec![pallet_evm::Event::Executed { address: checker }.into()]);
	});
}

#[test]
fn proxy_should_be_initialized_correctly() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let proxy = deploy_contract("TreasuryProxy", deployer());
		let mut controller_code = get_contract_bytecode("Treasury-Controller");
		controller_code.extend_from_slice(H256::from(deployer()).as_bytes());
		let controller = deploy_contract_code(controller_code, deployer());
		let implementation = deploy_contract("Treasury-Implementation", deployer());
		let implementation_init = EvmDataWriter::new_with_selector(Function::Initialize)
			.write(H256::from(EvmAddress::default()))
			.build();
		let (res, _) = Executor::<Runtime>::call(
			CallContext {
				contract: implementation,
				sender: deployer(),
				origin: deployer(),
			},
			implementation_init,
			U256::zero(),
			100_000,
		);
		assert_eq!(res, Succeed(Stopped), "Failed to initialize implementation");

		// Act
		let payload = EvmDataWriter::new_with_selector(Function::Initialize)
			.write(H256::from(controller))
			.build();
		let proxy_init = EvmDataWriter::new_with_selector(Function::InitializePayload)
			.write(H256::from(implementation))
			.write(H256::from(deployer()))
			.write(Bytes(payload))
			.build();
		let (res, _) = Executor::<Runtime>::call(
			CallContext {
				contract: proxy,
				sender: deployer(),
				origin: deployer(),
			},
			proxy_init,
			U256::zero(),
			1_000_000,
		);

		// Assert
		assert_eq!(res, Succeed(Stopped), "Failed to initialize proxy");
	});
}
