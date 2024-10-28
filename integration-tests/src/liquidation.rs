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
use sp_runtime::FixedPointNumber;
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;
use hydradx_traits::evm::EvmAddress;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use fp_evm::ExitReason::Succeed;
use fp_evm::ExitSucceed::Returned;
use hydradx_traits::evm::{CallContext, EVM};

const PATH_TO_SNAPSHOT: &str = "evm-snapshot/SNAPSHOT";

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
    GetPool = "getPool",
}

#[test]
fn liquidation() {
    TestNet::reset();
    hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
        // let mut storage = pallet_evm::AccountCodes::<hydradx_runtime::Runtime>::iter();
        // for i in storage {
        //     println!("------ {:?}", i.0);
        //
        // }

        let contract = EvmAddress::from_slice(hex!("82db570265c37bE24caf5bc943428a6848c3e9a6").as_slice());

        let data = Into::<u32>::into(Function::GetPool).to_be_bytes().to_vec();
        let context = CallContext::new_view(contract);

        let (res, value) = hydradx_runtime::evm::Executor::<hydradx_runtime::Runtime>::view(context, data, 500_000);

        // assert_eq!(res, Succeed(Returned));
        println!("---- {:?}", value);
    });
}
