use sp_std::cell::Cell;
use sp_std::marker::PhantomData;

use frame_support::traits::Contains;
use polkadot_xcm::v3::prelude::*;
use sp_core::Get;
use sp_runtime::Either;

/// Meant to serve as an `XcmExecuteFilter` for `pallet_xcm` by allowing XCM instructions related to transferring and
/// exchanging assets while disallowing e.g. `Transact`.
pub struct AllowTransferAndSwap<MaxXcmDepth, MaxInstructions, RuntimeCall>(
	PhantomData<(MaxXcmDepth, MaxInstructions, RuntimeCall)>,
);

impl<MaxXcmDepth, MaxInstructions, RuntimeCall> Contains<(MultiLocation, Xcm<RuntimeCall>)>
	for AllowTransferAndSwap<MaxXcmDepth, MaxInstructions, RuntimeCall>
where
	MaxXcmDepth: Get<u16>,
	MaxInstructions: Get<u16>,
{
	fn contains((loc, xcm): &(MultiLocation, Xcm<RuntimeCall>)) -> bool {
		// allow root to execute XCM
		if loc == &MultiLocation::here() {
			return true;
		}

		let instructions_count = Cell::new(0u16);
		check_instructions_recursively::<MaxXcmDepth, MaxInstructions, RuntimeCall>(xcm, 0, &instructions_count)
	}
}

/// Recurses depth-first through the instructions of an XCM and checks whether they are allowed, limiting both recursion
/// depth (via `MaxXcmDepth`) and instructions (`MaxInstructions`).
/// See [`allowed_or_recurse`] for the filter list.
fn check_instructions_recursively<MaxXcmDepth, MaxInstructions, RuntimeCall>(
	xcm: &Xcm<RuntimeCall>,
	depth: u16,
	instructions: &Cell<u16>,
) -> bool
where
	MaxXcmDepth: Get<u16>,
	MaxInstructions: Get<u16>,
{
	if depth > MaxXcmDepth::get() {
		return false;
	}
	for inst in xcm.inner().iter() {
		instructions.set(instructions.get() + 1);
		if instructions.get() > MaxInstructions::get() {
			return false;
		}

		match allowed_or_recurse(inst) {
			Either::Left(true) => continue,
			Either::Left(false) => return false,
			Either::Right(xcm) => {
				if !check_instructions_recursively::<MaxXcmDepth, MaxInstructions, ()>(xcm, depth + 1, instructions) {
					return false;
				}
			}
		}
	}
	true
}

/// Check if an XCM instruction is allowed (returning `Left(true)`), disallowed (`Left(false)`) or needs recursion to
/// determine whether it is allowed (`Right(xcm)`).
fn allowed_or_recurse<RuntimeCall>(inst: &Instruction<RuntimeCall>) -> Either<bool, &Xcm<()>> {
	match inst {
		ClearOrigin
		| ExchangeAsset { .. }
		| WithdrawAsset(..)
		| TransferAsset { .. }
		| DepositAsset { .. }
		| ExpectAsset(..)
		| SetFeesMode { .. }
		| BuyExecution { .. } => Either::Left(true),
		InitiateReserveWithdraw { xcm, .. } | DepositReserveAsset { xcm, .. } | TransferReserveAsset { xcm, .. } => {
			Either::Right(xcm)
		}
		_ => Either::Left(false),
	}
}
