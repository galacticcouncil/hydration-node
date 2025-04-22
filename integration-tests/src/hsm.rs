use crate::evm::dai_ethereum_address;
use crate::polkadot_test_net::hydra_live_ext;
use crate::polkadot_test_net::{Hydra, TestNet, ALICE, BOB, UNITS, WETH};
use crate::utils::contracts::{deploy_contract, deploy_contract_code, get_contract_bytecode};
use fp_evm::ExitSucceed::Returned;
use fp_evm::{ExitReason::Succeed, ExitSucceed::Stopped, FeeCalculator};
use frame_support::assert_ok;
use hex_literal::hex;
use hydradx_runtime::{
	evm::{
		precompiles::{handle::EvmDataWriter, Bytes},
		Executor,
	},
	AccountId, EVMAccounts, FixedU128, Runtime, RuntimeEvent, System, HSM,
};
use hydradx_traits::evm::{CallContext, EvmAddress, InspectEvmAccounts, EVM};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pretty_assertions::assert_eq;
use sp_core::{RuntimeDebug, H256, U256};
use sp_runtime::traits::One;
use sp_runtime::Perbill;
use sp_runtime::Permill;
use sp_runtime::SaturatedConversion;
use xcm_emulator::{Network, TestExt};

pub const PATH_TO_SNAPSHOT: &str = "snapshots/hsm/SNAPSHOT";

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	AddFacilitator = "addFacilitator(address,string,uint128)",
	Mint = "mint(address,uint256)",
	ListFacilitator = "getFacilitatorsList()",
	BalanceOf = "balanceOf(address)",
}

fn hollar_contract_address() -> EvmAddress {
	EvmAddress::from_slice(&hex!("C130c89F2b1066a77BD820AAFebCF4519D0103D8"))
}

fn hollar_contract_manager() -> EvmAddress {
	EvmAddress::from_slice(&hex!("52341e77341788Ebda44C8BcB4C8BD1B1913B204"))
}

fn minter() -> EvmAddress {
	EvmAddress::from_slice(&hex!("8f3ac7f6482abc1a5c48a95d97f7a235186dbb68"))
}

fn balance_of(address: EvmAddress) -> U256 {
	let context = CallContext::new_view(hollar_contract_address());
	let data = EvmDataWriter::new_with_selector(Function::BalanceOf)
		.write(address)
		.build();

	let (res, value) = Executor::<hydradx_runtime::Runtime>::view(context, data, 100_000);
	std::assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));
	sp_core::U256::from(value.as_slice())
}

fn list_facilitators() -> Vec<EvmAddress> {
	let data = Into::<u32>::into(Function::ListFacilitator).to_be_bytes().to_vec();
	let context = CallContext::new_view(hollar_contract_address());
	let (res, value) = Executor::<hydradx_runtime::Runtime>::view(context, data, 100_000);
	std::assert_eq!(res, Succeed(Returned), "{:?}", hex::encode(value));

	let mut r = vec![];
	for c in value.chunks(32) {
		r.push(EvmAddress::from(H256::from_slice(c)));
	}
	r
}

fn add_facilitator(facilitator: EvmAddress, label: &str, capacity: u128) {
	let context = CallContext::new_call(hollar_contract_address(), hollar_contract_manager());
	let data = EvmDataWriter::new_with_selector(Function::AddFacilitator)
		.write(facilitator)
		.write(Bytes::from(label))
		.write(capacity)
		.build();

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 5_000_000);
	std::assert_eq!(res, Succeed(Stopped), "{:?}", hex::encode(value));
}

fn mint(to: EvmAddress, amount: u128) {
	let context = CallContext::new_call(hollar_contract_address(), minter());
	let data = EvmDataWriter::new_with_selector(Function::Mint)
		.write(to)
		.write(amount)
		.build();

	let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 5_000_000);
	std::assert_eq!(res, Succeed(Stopped), "{:?}", hex::encode(value));
}

#[test]
fn add_hsm_facilitator_should_work() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let hsm_address = hydradx_runtime::HSM::account_id();
		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			hsm_address.clone().into()
		)));
		let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
		add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

		let facilitators = list_facilitators();
		assert!(facilitators.contains(&hsm_evm_address), "Facilitator not added");
	});
}

#[test]
fn buying_hollar_from_hsm_should_work() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let hsm_address = hydradx_runtime::HSM::account_id();
		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			hsm_address.clone().into()
		)));
		let hsm_evm_address = EVMAccounts::evm_address(&hsm_address);
		add_facilitator(hsm_evm_address, "hsm", 1_000_000_000_000_000_000_000);

		assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
			ALICE.into()
		),));
		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));
		mint(alice_evm_address, 1_500_000_000_000_000_000);
		let alice_hollar_balance = balance_of(alice_evm_address);
		assert_eq!(alice_hollar_balance, U256::from(1_500_000_000_000_000_000u128));

		assert_ok!(HSM::add_collateral_asset(
			hydradx_runtime::RuntimeOrigin::root(),
			100,
			10,
			Permill::zero(),
			FixedU128::one(),
			Permill::zero(),
			Perbill::one(),
			None
		));
	});
}

#[test]
#[ignore]
fn deploy_gho_token_should_work() {
	TestNet::reset();
	crate::polkadot_test_net::Hydra::execute_with(|| {
		let admin_evm: EvmAddress = hex!["52341e77341788Ebda44C8BcB4C8BD1B1913B204"].into();
		let _gho_contract_addr = crate::utils::contracts::deploy_contract("GhoToken", admin_evm);
	});
}
