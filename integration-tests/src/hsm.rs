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

#[test]
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
