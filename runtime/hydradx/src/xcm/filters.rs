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
		| SetFeesMode { .. }
		| BuyExecution { .. } => Either::Left(true),
		InitiateReserveWithdraw { xcm, .. } | DepositReserveAsset { xcm, .. } | TransferReserveAsset { xcm, .. } => {
			Either::Right(xcm)
		}
		_ => Either::Left(false),
	}
}

fn check_instructions_recursively<RuntimeCall>(xcm: &Xcm<RuntimeCall>, depth: u16, instructions: &Cell<usize>) -> bool {
	if depth >= 6 {
		return false;
	}

	// TODO: make configurable?
	let limit_per_level = 10; // TODO: make configurable?
	let max_instructions = 100usize; // TODO: make configurable?
	let depth_count = Cell::new(0usize);
	let mut instructions_count = instructions;
	let mut iter = xcm.inner().iter();
	while let (true, Some(inst)) = (
		/*depth_count.get() < limit_per_level,*/
		instructions_count.get() <= max_instructions,
		iter.next(),
	) {
		/*depth_count.set(depth_count.get() + 1);*/
		instructions_count.set(instructions_count.get() + 1);
		if instructions_count.get() > max_instructions {
			return false;
		}

		match allowed_or_recurse(inst) {
			Either::Left(true) => continue,
			Either::Left(false) => return false,
			Either::Right(xcm) => {
				if check_instructions_recursively(xcm, depth + 1, instructions_count) {
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

		let instructions = Cell::new(0usize);
		check_instructions_recursively(xcm, 0, &instructions)
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

	#[test]
	fn allow_transfer_and_swap_should_allow_a_transfer_and_swap() {
		//Arrange
		let fees = MultiAsset::from((MultiLocation::here(), 10));
		let weight_limit = WeightLimit::Unlimited;
		let give: MultiAssetFilter = fees.clone().into();
		let want: MultiAssets = fees.clone().into();
		let assets: MultiAssets = fees.clone().into();

		let max_assets = 2;
		let beneficiary = Junction::AccountId32 {
			id: [3; 32],
			network: None,
		}
		.into();
		let dest = MultiLocation::new(1, Parachain(2047));

		let xcm = Xcm(vec![
			BuyExecution { fees, weight_limit },
			ExchangeAsset {
				give,
				want,
				maximal: true,
			},
			DepositAsset {
				assets: Wild(AllCounted(max_assets)),
				beneficiary,
			},
		]);

		let message = Xcm(vec![
			SetFeesMode { jit_withdraw: true },
			TransferReserveAsset { assets, dest, xcm },
		]);

		let loc = MultiLocation::new(
			0,
			AccountId32 {
				network: None,
				id: [1; 32],
			},
		);

		//Act and assert
		assert!(AllowTransferAndSwap::<crate::RuntimeCall>::contains(&(loc, message)));
	}

	#[test]
	fn allow_transfer_and_swap_should_filter_too_deep_xcm() {
		//Arrange
		let fees = MultiAsset::from((MultiLocation::here(), 10));
		let assets: MultiAssets = fees.clone().into();

		let max_assets = 2;
		let beneficiary = Junction::AccountId32 {
			id: [3; 32],
			network: None,
		}
		.into();
		let dest = MultiLocation::new(1, Parachain(2047));

		let deposit = Xcm(vec![DepositAsset {
			assets: Wild(AllCounted(max_assets)),
			beneficiary,
		}]);

		let mut message = Xcm(vec![TransferReserveAsset {
			assets: assets.clone(),
			dest,
			xcm: deposit,
		}]);

		for _ in 0..5 {
			let xcm = message.clone();
			message = Xcm(vec![TransferReserveAsset {
				assets: assets.clone(),
				dest,
				xcm,
			}]);
		}

		let loc = MultiLocation::new(
			0,
			AccountId32 {
				network: None,
				id: [1; 32],
			},
		);

		//Act and assert
		assert!(!AllowTransferAndSwap::<()>::contains(&(loc, message)));
	}

	#[test]
	fn allow_transfer_and_swap_should_not_filter_message_with_max_deep() {
		//Arrange
		let fees = MultiAsset::from((MultiLocation::here(), 10));
		let assets: MultiAssets = fees.clone().into();

		let max_assets = 2;
		let beneficiary = Junction::AccountId32 {
			id: [3; 32],
			network: None,
		}
		.into();
		let dest = MultiLocation::new(1, Parachain(2047));

		let deposit = Xcm(vec![DepositAsset {
			assets: Wild(AllCounted(max_assets)),
			beneficiary,
		}]);

		let mut message = Xcm(vec![TransferReserveAsset {
			assets: assets.clone(),
			dest,
			xcm: deposit,
		}]);

		for _ in 0..4 {
			let xcm = message.clone();
			message = Xcm(vec![TransferReserveAsset {
				assets: assets.clone(),
				dest,
				xcm,
			}]);
		}

		let loc = MultiLocation::new(
			0,
			AccountId32 {
				network: None,
				id: [1; 32],
			},
		);

		//Act and assert
		assert!(AllowTransferAndSwap::<()>::contains(&(loc, message)));
	}

	#[test]
	fn allow_transfer_and_swap_should_filter_messages_with_one_more_instruction_than_allowed() {
		//Arrange
		let fees = MultiAsset::from((MultiLocation::here(), 10));
		let weight_limit = WeightLimit::Unlimited;
		let give: MultiAssetFilter = fees.clone().into();
		let want: MultiAssets = fees.clone().into();
		let assets: MultiAssets = fees.clone().into();

		let max_assets = 2;
		let beneficiary = Junction::AccountId32 {
			id: [3; 32],
			network: None,
		}
		.into();
		let dest = MultiLocation::new(1, Parachain(2047));

		let deposit = Xcm(vec![DepositAsset {
			assets: Wild(AllCounted(max_assets)),
			beneficiary,
		}]);

		let mut message = Xcm(vec![TransferReserveAsset {
			assets: assets.clone(),
			dest,
			xcm: deposit.clone(),
		}]);

		for _ in 0..2 {
			let xcm = message.clone();
			message = Xcm(vec![TransferReserveAsset {
				assets: assets.clone(),
				dest,
				xcm: xcm.clone(),
			}]);
		}

		//It has 5 instruction
		let mut instructions_with_inner_xcms: Vec<cumulus_primitives_core::Instruction<()>> =
			vec![TransferReserveAsset {
				assets: assets.clone(),
				dest,
				xcm: message.clone(),
			}];

		let mut rest: Vec<cumulus_primitives_core::Instruction<()>> = vec![
			DepositAsset {
				assets: Wild(AllCounted(max_assets)),
				beneficiary,
			};
			95
		];

		instructions_with_inner_xcms.append(&mut rest);

		message = Xcm(vec![TransferReserveAsset {
			assets: assets.clone(),
			dest,
			xcm: Xcm(instructions_with_inner_xcms.clone()),
		}]);

		let loc = MultiLocation::new(
			0,
			AccountId32 {
				network: None,
				id: [1; 32],
			},
		);

		//Act and assert
		assert!(!AllowTransferAndSwap::<()>::contains(&(loc, message)));
	}

	#[test]
	fn asd() {
		//Arrange
		let fees = MultiAsset::from((MultiLocation::here(), 10));
		let weight_limit = WeightLimit::Unlimited;
		let give: MultiAssetFilter = fees.clone().into();
		let want: MultiAssets = fees.clone().into();
		let assets: MultiAssets = fees.clone().into();

		let max_assets = 2;
		let beneficiary = Junction::AccountId32 {
			id: [3; 32],
			network: None,
		}
		.into();
		let dest = MultiLocation::new(1, Parachain(2047));

		let deposit = Xcm(vec![
			DepositAsset {
				assets: Wild(AllCounted(max_assets)),
				beneficiary,
			};
			100
		]);

		let mut message = Xcm(vec![TransferReserveAsset {
			assets: assets.clone(),
			dest,
			xcm: deposit.clone(),
		}]);

		let loc = MultiLocation::new(
			0,
			AccountId32 {
				network: None,
				id: [1; 32],
			},
		);

		//Act and assert
		assert!(!AllowTransferAndSwap::<()>::contains(&(loc, message)));
	}
}
