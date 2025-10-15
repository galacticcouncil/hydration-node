use crate::Config;
use codec::Decode;
use frame_support::traits::Get;
use pallet_evm::{ExitError, ExitReason};
use scale_info::prelude::string::String;
use sp_runtime::format;
use sp_runtime::DispatchError;
use sp_std::boxed::Box;
use sp_std::vec::Vec;

const ERROR_STRING_SELECTOR: [u8; 4] = [0x08, 0xC3, 0x79, 0xA0]; // Error(string)
const PANIC_SELECTOR: [u8; 4] = [0x4E, 0x48, 0x7B, 0x71]; // Panic(uint256)
const FUNCTION_SELECTOR_LENGTH: usize = 4;

pub trait EvmErrorDecoder {
	fn decode(call_result : CallResult) -> sp_runtime::DispatchError;
}

#[derive(Clone, Debug)]
pub struct CallResult {
	pub exit_reason: ExitReason,
	pub value: Vec<u8>,
	pub contract: sp_core::H160,
}

pub struct EvmErrorDecoderAdapter<T>(core::marker::PhantomData<T>) where T: Config;

impl<T: Config> EvmErrorDecoder for EvmErrorDecoderAdapter<T> {
	fn decode(call_result : CallResult) -> DispatchError {
		if let ExitReason::Error(ExitError::OutOfGas) = call_result.exit_reason {
			return crate::Error::<T>::EvmOutOfGas.into();
		}

		if call_result.value.len() < FUNCTION_SELECTOR_LENGTH {
			return dispatch_error_other(call_result.value);
		}

		//Try to decode as SCALE-encoded DispatchError from precompiles
		if let Ok(dispatch_error) = DispatchError::decode(&mut &call_result.value[..]) {
			return dispatch_error;
		}

		if call_result.value.starts_with(&PANIC_SELECTOR) && call_result.value.len() >= FUNCTION_SELECTOR_LENGTH + 32 {
			if call_result.value[35] == 0x11 {
				return return crate::Error::<T>::EvmArithmeticOverflowOrUnderflow.into();
			}
		}

		// Check for generic Error(string)
		if call_result.value.starts_with(&ERROR_STRING_SELECTOR) {
			// Check for AAVE-specific errors if contract matches
			if call_result.contract == T::BorrowingContract::get()
				&& call_result.value.len() >= 70
				&& call_result.value[66..68] == [0x00, 0x02] // string length = 2
			{
				match &call_result.value[68..70] {
					b"45" => return crate::Error::<T>::AaveHealthFactorNotBelowThreshold.into(),
					b"50" => return crate::Error::<T>::AaveBorrowCapExceeded.into(),
					b"51" => return crate::Error::<T>::AaveSupplyCapExceeded.into(),
					_ => {},
				}
			}
		}


		//TODO:
		//HEALTH_FACTOR_NOT_BELOW_THRESHOLD
		//check doc for more
		//And here too: https://github.com/aave/aave-v3-core/blob/782f51917056a53a2c228701058a6c3fb233684a/contracts/protocol/libraries/helpers/Errors.sol#L54

		dispatch_error_other(call_result.value)
	}
}

fn dispatch_error_other(value: Vec<u8>) -> DispatchError {
	DispatchError::Other(&*Box::leak(
		format!("evm:0x{}", hex::encode(&value)).into_boxed_str(),
	))
}



// ask about the pallet dependency
// review staged code again
// refactor and improve logic
