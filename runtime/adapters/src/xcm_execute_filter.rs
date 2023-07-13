use sp_std::cell::Cell;
use sp_std::marker::PhantomData;

use frame_support::traits::Contains;
use polkadot_xcm::v3::prelude::*;
use sp_core::Get;
use sp_runtime::Either;

pub struct AllowTransferAndSwap<MaxXcmDepth, MaxInstructions, RuntimeCall>(
	PhantomData<(MaxXcmDepth, MaxInstructions, RuntimeCall)>,
)
where
	MaxXcmDepth: Get<u16>,
	MaxInstructions: Get<u16>;

fn allowed_or_recurse<RuntimeCall>(inst: &Instruction<RuntimeCall>) -> Either<bool, &Xcm<()>> {
	match inst {
		ClearOrigin
		| ClaimAsset { .. }
		| ExchangeAsset { .. }
		| WithdrawAsset(..)
		| TransferAsset { .. }
		| DepositAsset { .. }
		| SetTopic(..)
		| ClearTopic
		| ExpectAsset(..)
		| BurnAsset(..)
		| SetFeesMode { .. }
		| BuyExecution { .. } => Either::Left(true),
		InitiateReserveWithdraw { xcm, .. } | DepositReserveAsset { xcm, .. } | TransferReserveAsset { xcm, .. } => {
			Either::Right(xcm)
		}
		_ => Either::Left(false),
	}
}

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
	let instructions_count = instructions; //TODO: use just a let mut u16
	let mut iter = xcm.inner().iter();
	while let Some(inst) = iter.next() {
		instructions_count.set(instructions_count.get() + 1);
		if instructions_count.get() > MaxInstructions::get() {
			return false;
		}

		match allowed_or_recurse(inst) {
			Either::Left(true) => continue,
			Either::Left(false) => return false,
			Either::Right(xcm) => {
				if check_instructions_recursively::<MaxXcmDepth, MaxInstructions, ()>(
					xcm,
					depth + 1,
					instructions_count,
				) {
					continue;
				} else {
					return false;
				}
			}
		}
	}
	true
}

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
