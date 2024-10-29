#![cfg(test)]

use ethabi::ethereum_types::H160;
use fp_evm::ExitSucceed;
use crate::polkadot_test_net::*;

use frame_support::{
    assert_noop, assert_ok,
    sp_runtime::RuntimeDebug,
};
use hex_literal::hex;
use orml_traits::currency::MultiCurrency;
use orml_traits::MultiCurrencyExtended;
use sp_runtime::{FixedPointNumber, SaturatedConversion};
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;
use hydradx_traits::evm::EvmAddress;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use fp_evm::ExitReason::Succeed;
use fp_evm::ExitSucceed::Returned;
use hydradx_traits::evm::{CallContext, EVM, Erc20Mapping};
use hydradx_runtime::{
    EVMAccounts, RuntimeOrigin,
    evm::Executor,
};
use sp_core::{H256, U256};
use hydradx_runtime::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use ethabi::ethereum_types::BigEndianHash;

const PATH_TO_SNAPSHOT: &str = "evm-snapshot/SNAPSHOT";

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
    GetPool = "getPool()",
    GetReservesList = "getReservesList()",
    Supply = "supply(address,uint256,address,uint16)",
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
        let supply: Balance = 1_000 * 10_000_000_000; // 1M DOT, 10 decimals
        assert_ok!(Currencies::deposit(dot_asset, &ALICE.into(), amount));

        assert_ok!(
			EVMAccounts::bind_evm_address(
				RuntimeOrigin::signed(ALICE.into()),
			)
		);
        let alice_evm_address = EVMAccounts::evm_address(&AccountId::from(ALICE));

        let context = CallContext::new_call(pool_contract, alice_evm_address);
        let mut data = Into::<u32>::into(Function::Supply).to_be_bytes().to_vec();
        data.extend_from_slice(H256::from(dot_asset_address).as_bytes());
        data.extend_from_slice(H256::from_uint(&U256::from(supply.saturated_into::<u128>())).as_bytes());
        data.extend_from_slice(H256::from(alice_evm_address).as_bytes());
        data.extend_from_slice(H256::zero().as_bytes());
        let (res, value) = Executor::<hydradx_runtime::Runtime>::call(context, data, U256::zero(), 100_000);
        println!("---- {:?}", res);
        println!("---- {:X?}", value);
        assert_eq!(res, Succeed(Returned));
    });
}
