use crate::Liquidation;
use codec::Decode;
use frame_support::traits::Get;
use hydradx_traits::evm::CallResult;
use pallet_evm::{ExitError, ExitReason};
use sp_runtime::format;
use sp_runtime::traits::Convert;
use sp_runtime::DispatchError;
use sp_std::boxed::Box;
use sp_std::vec::Vec;

const ERROR_STRING_SELECTOR: [u8; 4] = [0x08, 0xC3, 0x79, 0xA0]; // Error(string)
const PANIC_SELECTOR: [u8; 4] = [0x4E, 0x48, 0x7B, 0x71]; // Panic(uint256)
const FUNCTION_SELECTOR_LENGTH: usize = 4;

pub struct EvmErrorDecoder;

impl Convert<CallResult, DispatchError> for EvmErrorDecoder {
	fn convert(call_result: CallResult) -> DispatchError {
		if let ExitReason::Error(ExitError::OutOfGas) = call_result.exit_reason {
			return pallet_dispatcher::Error::<crate::Runtime>::EvmOutOfGas.into();
		}

		//Check for data without valid function selector
		if call_result.value.len() < FUNCTION_SELECTOR_LENGTH {
			return dispatch_error_other(call_result.value);
		}

		//Try to decode as SCALE-encoded DispatchError from precompiles
		if let Ok(dispatch_error) = DispatchError::decode(&mut &call_result.value[..]) {
			return dispatch_error;
		}

		// Check for Panic(uint256)
		if call_result.value.starts_with(&PANIC_SELECTOR) && call_result.value.len() >= FUNCTION_SELECTOR_LENGTH + 32 {
			if call_result.value.get(35) == Some(&0x11) {
				return pallet_dispatcher::Error::<crate::Runtime>::EvmArithmeticOverflowOrUnderflow.into();
			}
		}

		// Check for generic Error(string)
		if call_result.value.starts_with(&ERROR_STRING_SELECTOR) {
			// Check for AAVE-specific errors if contract matches
			// Error string length must be 2 (&[0x00, 0x02])
			if call_result.contract == crate::Liquidation::get()
				&& call_result.value.len() >= 70
				&& call_result.value.get(66..68) == Some(&[0x00, 0x02])
			{
				if let Some(error_code) = call_result.value.get(68..70) {
					match error_code {
						b"35" => return pallet_dispatcher::Error::<crate::Runtime>::AaveHealthFactorLowerThanLiquidationThreshold.into(),
						b"36" => return pallet_dispatcher::Error::<crate::Runtime>::CollateralCannotCoverNewBorrow.into(),
						b"45" => return pallet_dispatcher::Error::<crate::Runtime>::AaveHealthFactorNotBelowThreshold.into(),
						b"50" => return pallet_dispatcher::Error::<crate::Runtime>::AaveBorrowCapExceeded.into(),
						b"51" => return pallet_dispatcher::Error::<crate::Runtime>::AaveSupplyCapExceeded.into(),
						_ => {},
					}
				}
			}
		}

		dispatch_error_other(call_result.value)
	}
}

fn dispatch_error_other(value: Vec<u8>) -> DispatchError {
	let error_string = format!("evm:0x{}", hex::encode(&value));
	DispatchError::Other(Box::leak(error_string.into_boxed_str()))
}
