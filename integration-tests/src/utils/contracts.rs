use fp_rpc::runtime_decl_for_ethereum_runtime_rpc_api::EthereumRuntimeRPCApiV5;
use frame_support::assert_ok;
use hydradx_runtime::EVMAccounts;
use hydradx_traits::evm::EvmAddress;
use pallet_evm::ExitReason;
use sp_core::U256;
use std::fs;

pub fn get_contract_bytecode(name: &str) -> Vec<u8> {
	let path = format!(
		"../scripts/test-contracts/artifacts/contracts/{}.sol/{}.json",
		name, name
	);
	let str = fs::read_to_string(path).unwrap();
	let json: serde_json::Value = serde_json::from_str(&str).unwrap();
	let code = json.get("bytecode").unwrap().as_str().unwrap();
	hex::decode(&code[2..]).unwrap()
}

pub fn deploy_contract_code(code: Vec<u8>, deployer: EvmAddress) -> EvmAddress {
	assert_ok!(EVMAccounts::add_contract_deployer(
		hydradx_runtime::RuntimeOrigin::root(),
		deployer,
	));

	let info = hydradx_runtime::Runtime::create(
		deployer,
		code.clone(),
		U256::zero(),
		U256::from(2000000u64),
		None,
		None,
		None,
		false,
		None,
	);

	let address = match info.clone().unwrap().exit_reason {
		ExitReason::Succeed(_) => info.unwrap().value,
		reason => panic!("{:?}", reason),
	};

	let deployed = hydradx_runtime::Runtime::account_code_at(address.clone());
	assert_ne!(deployed, vec![0; deployed.len()]);
	address
}

pub fn deploy_contract(name: &str, deployer: EvmAddress) -> EvmAddress {
	deploy_contract_code(get_contract_bytecode(name), deployer)
}
