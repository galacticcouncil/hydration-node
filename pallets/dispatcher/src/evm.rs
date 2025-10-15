use crate::Config;
use codec::Decode;
use frame_support::traits::Get;
use pallet_evm::{ExitError, ExitReason};
use scale_info::prelude::string::String;
use sp_runtime::format;
use sp_runtime::DispatchError;
use sp_std::boxed::Box;
use sp_std::vec::Vec;

//TODO: rename to decoder, also the adapter
pub trait EvmErrorDecoder {
	fn decode(call_result : CallResult) -> sp_runtime::DispatchError;
}

#[derive(Clone, Debug)]
pub struct CallResult {
	pub exit_reason: ExitReason,
	pub value: Vec<u8>,
	pub contract: sp_core::H160,
}

const ERROR_STRING: [u8; 4] = [0x08, 0xC3, 0x79, 0xA0]; // Error(string)
const PANIC: [u8; 4] = [0x4E, 0x48, 0x7B, 0x71]; // Panic(uint256)

pub struct EvmErrorDecoderAdapter<T>(core::marker::PhantomData<T>) where T: Config;

impl<T: Config> EvmErrorDecoder for EvmErrorDecoderAdapter<T> {
	fn decode(call_result : CallResult) -> DispatchError {
		if let ExitReason::Error(ExitError::OutOfGas) = call_result.exit_reason {
			return crate::Error::<T>::EvmOutOfGas.into();
		}

		//Try to decode as SCALE-encoded DispatchError from precompiles
		if let Ok(dispatch_error) = DispatchError::decode(&mut &call_result.value[..]) {
			return dispatch_error;
		}

		//TODO: check against panic

		//TODO: verify if this ever happens, by returning some other error
		// if self.value.len() < 4 {
		//     return b"Revert".to_vec();
		//       other VM errors → hex dump raw
		//return Err(leak(&format!("evm:0x{}", hex::encode(data))));
		// }

		if call_result.value.starts_with(&PANIC) && call_result.value.len() >= 4 + 32 {
			if call_result.value[35] == 0x11 {
				//TODO: isnt it 17 the panic code? Lumir says so
				return return crate::Error::<T>::EvmArithmeticOverflowOrUnderflow.into();
			}
		}

		// Check for AAVE-specific errors if contract matches
		if call_result.contract == T::BorrowingContract::get() && call_result.value.starts_with(&ERROR_STRING) {
			// Error(string) encoding:
			// 0x08c379a0 (4 bytes) - Error selector
			// 0x0000...0020 (32 bytes) - offset to string data
			// 0x0000...00XX (32 bytes) - string length
			// actual string data (padded to 32 bytes)

			if call_result.value.len() >= 68 {
				// Extract string length from the last 4 bytes of the 32-byte length word (big-endian)
				let string_length = u32::from_be_bytes(
					call_result.value[64..68].try_into().unwrap_or([0u8; 4])
				) as usize;

				// Ensure we have enough data and the length is reasonable
				if string_length > 0 && string_length <= 100 && call_result.value.len() >= 68 + string_length {
					// Extract the actual error string starting at byte 68
					let error_code = &call_result.value[68..68 + string_length];

					//Add
					//COLLATERAL_CANNOT_COVER_NEW_BORROW
					//AMOUNT_TOO_BIG check for this
					match error_code {
						b"45" => return crate::Error::<T>::AaveHealthFactorNotBelowThreshold.into(),
						b"50" => return crate::Error::<T>::AaveBorrowCapExceeded.into(),
						b"51" => return crate::Error::<T>::AaveSupplyCapExceeded.into(),
						_ => {},
					}
				}
			}
		}

		//TODO:
		//HEALTH_FACTOR_NOT_BELOW_THRESHOLD
		//SUPPLY_CAP
		//check doc for more
		//And here too: https://github.com/aave/aave-v3-core/blob/782f51917056a53a2c228701058a6c3fb233684a/contracts/protocol/libraries/helpers/Errors.sol#L54

		// Check for old-style revert with Error(string)
		if call_result.value.starts_with(&ERROR_STRING) {
			// ABI-encoded string starts after the first 4 bytes
			// Format:
			// 0x08c379a0
			// 32 bytes offset (usually 0x20)
			// 32 bytes string length (N)
			// N bytes string data (UTF-8 text)
			// optional padding

			// Example extraction:
			if call_result.value.len() >= 4 + 64 {
				// return DispatchError::Other(&*Box::leak(
				//     format!("evm:0x{}", hex::encode(&self.value)).into_boxed_str(),
				// ));

				// selector is at [0..4]; offset word is at [4..36]
				// take the LAST 4 bytes of the 32B offset word (big-endian)
				let offset = u32::from_be_bytes(call_result.value[32..36].try_into().unwrap_or([0u8; 4])) as usize;

				let len_pos = 4 + offset; // position of the length word
				if call_result.value.len() >= len_pos + 32 {
					// take the LAST 4 bytes of the 32B length word (big-endian)
					let len = u32::from_be_bytes(call_result.value[len_pos + 28..len_pos + 32].try_into().unwrap_or([0u8; 4]))
						as usize;

					let start = len_pos + 32; // start of the string bytes
					let end = start.saturating_add(len).min(call_result.value.len());

					if end >= start {
						let message = &call_result.value[start..end];
						let revert_message = format!("evm revert: {}", String::from_utf8_lossy(message)).into_bytes();

						return DispatchError::Other(&*Box::leak(
							format!("evm:0x{}", hex::encode(&revert_message)).into_boxed_str(),
						));
					}
				}
			}
		}

		//TODO: check if we can do other error handling here.
		DispatchError::Other(&*Box::leak(
			format!("evm:0x{}", hex::encode(&call_result.value)).into_boxed_str(),
		))
	}
}



// ask about the pallet dependency
// review staged code again
// refactor and improve logic
