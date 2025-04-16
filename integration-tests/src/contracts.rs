use crate::evm::dai_ethereum_address;
use crate::polkadot_test_net::{Hydra, TestNet, ALICE, BOB, UNITS, WETH};
use crate::utils::contracts::{deploy_contract, deploy_contract_code, get_contract_bytecode};
use fp_evm::{ExitReason::Succeed, ExitSucceed::Stopped, FeeCalculator};
use frame_support::assert_ok;
use hex_literal::hex;
use hydradx_runtime::{
	evm::{
		precompiles::{handle::EvmDataWriter, Bytes},
		Executor,
	},
	AccountId, EVMAccounts, Runtime, RuntimeEvent, System,
};
use hydradx_traits::evm::{CallContext, EvmAddress, InspectEvmAccounts, EVM};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pretty_assertions::assert_eq;
use sp_core::{RuntimeDebug, H256, U256};
use test_utils::expect_events;
use xcm_emulator::{Network, TestExt};

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
		pallet_evm::Pallet::<Runtime>::remove_account(&dai_ethereum_address());
		assert_eq!(is_contract(checker, dai_ethereum_address()), false);
	});
}

#[test]
fn contract_check_succeeds_on_precompile_with_invalid_code() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let checker = deploy_contract("ContractCheck", deployer());
		// The code is invalid, but we intentionally set account codes of registered assets to 0.
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
