use crate::evm::dai_ethereum_address;
use crate::polkadot_test_net::hydra_live_ext;
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
use sp_runtime::SaturatedConversion;
use test_utils::expect_events;
use xcm_emulator::{Network, TestExt};

pub const PATH_TO_SNAPSHOT: &str = "snapshots/hsm/SNAPSHOT";

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	AddFacilitator = "addFacilitator(address,string,uint128)",
}

pub fn add_facilitator(facilitator: EvmAddress, capacity: u128) {
	let caller = EvmAddress::from_slice(&hex!("3dC06FAA422A0Cf6014847031dDc1DeC7B63F76a"));
	let hollar_address = EvmAddress::from_slice(&hex!("C130c89F2b1066a77BD820AAFebCF4519D0103D8"));
	let context = CallContext::new_call(hollar_address, caller);
	let label = "hsm";
	let b = Bytes::from(label);

	let data = EvmDataWriter::new_with_selector(Function::AddFacilitator)
		.write(facilitator)
		.write(b)
		.write(capacity)
		.build();

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 5_000_000);
	std::assert_eq!(res, Succeed(Stopped), "{:?}", hex::encode(value));
}

#[test]
fn add_hsm_facilitator_should_work() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let hollar_address = EvmAddress::from_slice(&hex!("C130c89F2b1066a77BD820AAFebCF4519D0103D8"));
		let code_meta  = pallet_evm::AccountCodesMetadata::<hydradx_runtime::Runtime>::get(&hollar_address);
		assert!(code_meta.is_some());
		let hsm_address = hydradx_runtime::HSM::account_id();
		let hsm_evm_address = hydradx_runtime::EVMAccounts::evm_address((&hsm_address).into());
		add_facilitator(hsm_evm_address.clone(), 1_000_000_000_000_000_000_000_000);
	});
}

/*
#[test]
#[ignore]
fn deploy_gho_token_should_work() {
	TestNet::reset();
	crate::polkadot_test_net::Hydra::execute_with(|| {
		let admin_evm: EvmAddress = hex!["3dC06FAA422A0Cf6014847031dDc1DeC7B63F76a"].into();
		let admin_acc = hydradx_runtime::EVMAccounts::truncated_account_id(admin_evm.clone());

		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			admin_acc.clone().into(),
			WETH,
			(10_000_000 * UNITS) as i128,
		));
		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			hydradx_runtime::RuntimeOrigin::signed(admin_acc.into()),
			WETH
		));
		//let gho_contract_addr= crate::utils::contracts::deploy_contract("GhoToken", crate::contracts::deployer());
		let gho_contract_addr = crate::utils::contracts::deploy_contract("GhoToken", admin_evm);
	});
}
 */
