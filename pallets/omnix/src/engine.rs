use crate::types::{Balance, BoundedInstructions, BoundedResolvedIntents, Solution};
use crate::Config;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::BoundedVec;
use frame_support::__private::RuntimeDebug;
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::{TypeInfo, Weight};
use frame_support::traits::fungibles::Mutate;
use frame_support::traits::tokens::Preservation;
use frame_support::traits::OriginTrait;
use hydradx_traits::router::{AmountInAndOut, RouterT, Trade};
use sp_core::ConstU32;
use sp_runtime::DispatchError;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum Instruction<AccountId, AssetId> {
	TransferIn {
		who: AccountId,
		asset_id: AssetId,
		amount: Balance,
	},
	TransferOut {
		who: AccountId,
		asset_id: AssetId,
		amount: Balance,
	},
	SwapExactIn {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		amount_out: Balance,
		route: BoundedVec<Trade<AssetId>, ConstU32<5>>,
	},
	SwapExactOut {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		amount_out: Balance,
		route: BoundedVec<Trade<AssetId>, ConstU32<5>>,
	},
}

pub struct OmniXEngine<T, C, R>(sp_std::marker::PhantomData<(T, C, R)>);

impl<T: Config, C, R> OmniXEngine<T, C, R>
where
	C: Mutate<T::AccountId, AssetId = T::AssetId, Balance = Balance>,
	R: RouterT<T::RuntimeOrigin, T::AssetId, Balance, Trade<T::AssetId>, AmountInAndOut<Balance>>,
{
	pub fn validate_solution(solution: &mut Solution<T::AccountId, T::AssetId>) -> Result<(), DispatchError> {
		//TODO: check balances, limits, partials etc?!

		solution.weight = Self::calculate_weight(&solution.instructions)?;

		Ok(())
	}

	fn calculate_weight(
		_instructions: &BoundedInstructions<T::AccountId, T::AssetId>,
	) -> Result<Weight, DispatchError> {
		Ok(Weight::default())
	}

	fn update_intents(_resolved_intents: BoundedResolvedIntents) -> DispatchResult {
		//TODO: update intent or remove it if completely resolved
		Ok(())
	}

	pub fn execute_solution(solution: Solution<T::AccountId, T::AssetId>) -> Result<(), DispatchError> {
		let holding_account = crate::Pallet::<T>::holding_account();

		for instruction in solution.instructions {
			match instruction {
				Instruction::TransferIn { who, asset_id, amount } => {
					C::transfer(asset_id, &who, &holding_account, amount, Preservation::Expendable)?;
				}
				Instruction::TransferOut { who, asset_id, amount } => {
					C::transfer(asset_id, &holding_account, &who, amount, Preservation::Expendable)?;
				}
				Instruction::SwapExactIn {
					asset_in,
					asset_out,
					amount_in,
					amount_out,
					route,
				} => {
					let origin = T::RuntimeOrigin::signed(holding_account.clone().into());
					R::sell(
						origin,
						asset_in,
						asset_out,
						amount_in,
						amount_out, // set as limit in the instruction
						route.to_vec(),
					)?;
				}
				Instruction::SwapExactOut {
					asset_in,
					asset_out,
					amount_in,
					amount_out,
					route,
				} => {
					let origin = T::RuntimeOrigin::signed(holding_account.clone().into());
					R::buy(
						origin,
						asset_in,
						asset_out,
						amount_in,
						amount_out, // set as limit in the instruction
						route.to_vec(),
					)?;
				}
			}
		}

		Self::update_intents(solution.intents)?;

		Ok(())
	}
}
