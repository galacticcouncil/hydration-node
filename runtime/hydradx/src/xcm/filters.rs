use core::{cell::Cell, marker::PhantomData};

use frame_support::traits::Contains;
use polkadot_xcm::v3::prelude::*;
use sp_core::{ConstU16, Get};
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
	instructions: &Cell<u16>, //TODO: don't use std, use core or sp_std for Cell
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

#[cfg(test)]
mod tests {
	use super::*;

	use codec::Encode;
	use frame_support::pallet_prelude::Weight;

	//TODO: consider what others needs to be filtered out then add them to this test
	#[test]
	fn xcm_execute_filter_should_not_allow_transact() {
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
		assert!(xcm_execute_filter_does_not_allow(&(loc, xcm)));
	}

	#[test]
	fn xcm_execute_filter_should_allow_a_transfer_and_swap() {
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
		assert!(xcm_execute_filter_allows(&(loc, message)));
	}

	#[test]
	fn xcm_execute_filter_should_filter_too_deep_xcm() {
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
		assert!(xcm_execute_filter_does_not_allow(&(loc, message)));
	}

	#[test]
	fn xcm_execute_filter_should_not_filter_message_with_max_deep() {
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
		assert!(AllowTransferAndSwap::<ConstU16<5>, ConstU16<100>, ()>::contains(&(
			loc, message
		)));
	}

	#[test]
	fn xcm_execute_filter_should_filter_messages_with_one_more_instruction_than_allowed_in_depth() {
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
		assert!(xcm_execute_filter_does_not_allow(&(loc, message)));
	}

	#[test]
	fn xcm_execute_filter_should_filter_messages_with_one_more_instruction_than_allowed_in_one_level() {
		//Arrange
		let max_assets = 2;
		let beneficiary = Junction::AccountId32 {
			id: [3; 32],
			network: None,
		}
		.into();

		let message_with_more_instructions_than_allowed = Xcm(vec![
			DepositAsset {
				assets: Wild(AllCounted(max_assets)),
				beneficiary,
			};
			101
		]);

		let loc = MultiLocation::new(
			0,
			AccountId32 {
				network: None,
				id: [1; 32],
			},
		);

		//Act and assert
		assert!(xcm_execute_filter_does_not_allow(&(
			loc,
			message_with_more_instructions_than_allowed
		)));
	}

	fn xcm_execute_filter_allows(loc_and_message: &(MultiLocation, Xcm<crate::RuntimeCall>)) -> bool {
		AllowTransferAndSwap::<ConstU16<5>, ConstU16<100>, crate::RuntimeCall>::contains(loc_and_message)
	}

	fn xcm_execute_filter_does_not_allow(loc_and_message: &(MultiLocation, Xcm<()>)) -> bool {
		!AllowTransferAndSwap::<ConstU16<5>, ConstU16<100>, ()>::contains(loc_and_message)
	}
}
