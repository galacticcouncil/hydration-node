#![cfg(test)]

use crate::polkadot_test_net::*;

use ethabi::ethereum_types::BigEndianHash;
use fp_evm::ExitReason::Succeed;
use fp_evm::ExitSucceed::Returned;
use frame_support::{assert_ok, sp_runtime::RuntimeDebug};
use hex_literal::hex;
use hydradx_runtime::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use hydradx_runtime::{evm::Executor, EVMAccounts, RuntimeOrigin};
use hydradx_traits::evm::EvmAddress;
use hydradx_traits::evm::{CallContext, Erc20Mapping, EVM};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use orml_traits::currency::MultiCurrency;
use sp_core::{H256, U256};
use sp_runtime::SaturatedConversion;

const PATH_TO_SNAPSHOT: &str = "evm-snapshot/SNAPSHOT";

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	GetPool = "getPool()",
	GetReservesList = "getReservesList()",
	Supply = "supply(address,uint256,address,uint16)",
	Withdraw = "withdraw(address,uint256,address)",
}

#[test]
fn liquidation() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		// PoolAddressesProvider contract
		let pap_contract = EvmAddress::from_slice(hex!("82db570265c37bE24caf5bc943428a6848c3e9a6").as_slice());

		// get Pool contract address
		let data = Into::<u32>::into(Function::GetPool).to_be_bytes().to_vec();
		let context = CallContext::new_view(pap_contract);

		let (res, value) = Executor::<hydradx_runtime::Runtime>::view(context, data, 100_000);
		let pool_contract: EvmAddress = EvmAddress::from(H256::from_slice(&value));
		println!("POOL contract: {:X?}", pool_contract);
		assert_eq!(res, Succeed(Returned));

		let data = Into::<u32>::into(Function::GetReservesList).to_be_bytes().to_vec();
		let context = CallContext::new_view(pool_contract);
		let (res, value) = Executor::<hydradx_runtime::Runtime>::view(context, data, 100_000);
		assert_eq!(res, Succeed(Returned));

		// DOT: AssetId = 5
		let dot_asset_address = EvmAddress::from(H256::from_slice(&value[192..224]));
		let dot_asset = HydraErc20Mapping::decode_evm_address(dot_asset_address).unwrap();

		let amount: Balance = 1_000_000 * 10_000_000_000; // 1M DOT, 10 decimals
		let supply: Balance = 50000000000;

		assert_ok!(Currencies::deposit(dot_asset, &ALICE.into(), amount));
		assert_ok!(Currencies::deposit(HDX, &ALICE.into(), amount));
		assert_ok!(Currencies::deposit(WETH, &ALICE.into(), 100_000 * amount));

		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));
		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pap_contract));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into()),));
		let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

		let context = CallContext::new_call(pool_contract, alice_evm_address);
		let mut data = Into::<u32>::into(Function::Supply).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(dot_asset_address).as_bytes());
		data.extend_from_slice(H256::from_uint(&U256::from(supply.saturated_into::<u128>())).as_bytes());
		data.extend_from_slice(H256::from(alice_evm_address).as_bytes());
		data.extend_from_slice(H256::zero().as_bytes());

		println!("asset        {:X?}", H256::from(dot_asset_address).as_bytes());
		println!(
			"amount       {:X?}",
			H256::from_uint(&U256::from(supply.saturated_into::<u128>())).as_bytes()
		);
		println!("onBehalfOf   {:X?}", H256::from(alice_evm_address).as_bytes());
		println!("referralCode {:X?}", H256::zero().as_bytes());
		println!("data         {:X?}", hex::encode(data.clone()));

		let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 100_000);
		println!("---- {:X?}", res);
		println!("---- {:X?}", hex::encode(value));
		assert_eq!(res, Succeed(Returned));
	});
}
