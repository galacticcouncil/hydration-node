use core::{cell::Cell, marker::PhantomData};

use frame_support::traits::Contains;
use polkadot_xcm::v3::prelude::*;
use sp_runtime::Either;

pub struct AllowTransferAndSwap<RuntimeCall>(PhantomData<RuntimeCall>);

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
		| BuyExecution { .. } => Either::Left(true),
		InitiateReserveWithdraw { xcm, .. } | DepositReserveAsset { xcm, .. } | TransferReserveAsset { xcm, .. } => {
			Either::Right(xcm)
		}
		_ => Either::Left(false),
	}
}

fn check_instructions_recursively<RuntimeCall>(xcm: &Xcm<RuntimeCall>, depth: u16) -> bool {
	if depth > 6 {
		return false;
	} // TODO: make configurable?
	let limit = 10; // TODO: make configurable?
	let count = Cell::new(0usize);
	let mut iter = xcm.inner().iter();
	while let (true, Some(inst)) = (count.get() < limit, iter.next()) {
		count.set(count.get() + 1);
		match allowed_or_recurse(inst) {
			Either::Left(true) => continue,
			Either::Left(false) => return false,
			Either::Right(xcm) => {
				if check_instructions_recursively(xcm, depth + 1) {
					continue;
				} else {
					return false;
				}
			}
		}
	}
	true
}

impl<RuntimeCall> Contains<(MultiLocation, Xcm<RuntimeCall>)> for AllowTransferAndSwap<RuntimeCall> {
	fn contains((loc, xcm): &(MultiLocation, Xcm<RuntimeCall>)) -> bool {
		// allow root to execute XCM
		if loc == &MultiLocation::here() {
			return true;
		}
		check_instructions_recursively(xcm, 0)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use codec::Encode;
	use frame_support::pallet_prelude::Weight;

	#[test]
	fn allow_transfer_and_swap_should_filter_transact() {
		let call = crate::RuntimeCall::System(frame_system::Call::remark { remark: Vec::new() }).encode();
		let xcm = Xcm(vec![Transact {
			origin_kind: OriginKind::Native,
			require_weight_at_most: Weight::from_parts(1, 1),
			call: call.into(),
		}]);
		let loc = MultiLocation::new(
			0,
			AccountId32 {
				network: None,
				id: [1; 32],
			},
		);
		assert!(!AllowTransferAndSwap::<crate::RuntimeCall>::contains(&(loc, xcm)));
	}
}
