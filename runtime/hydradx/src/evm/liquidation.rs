use hydradx_runtime::evm::{
    precompiles::handle::EvmDataWriter
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use sp_core::{H256, U256};

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	LiquidationCall = "liquidationCall(address,address,address,uint256,bool)",
}

fn liquidation_call_context(collateral_asset: H256, debt_asset: H256, user: H256, debt_to_cover: U256, receive_atoken: bool) {
    let call_context = EvmDataWriter::new_with_selector(Function::LiquidationCall)
        .write(collateral_asset)
        .write(debt_asset)
        .write(user)
        .write(debt_to_cover)
        .write(receive_atoken)
        .build();
}