use crate::pallet::Intents;
use crate::types::{
	Balance, BoundedInstructions, BoundedResolvedIntents, Intent, ResolvedIntent, Solution, Swap, SwapType,
};
use crate::{Config, Error};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::__private::RuntimeDebug;
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::{ConstU32, Get, TypeInfo, Weight};
use frame_support::traits::fungibles::Mutate;
use frame_support::traits::tokens::Preservation;
use frame_support::traits::OriginTrait;
use frame_support::{ensure, BoundedVec};
use hydradx_traits::router::{AmountInAndOut, RouterT, Trade};
use orml_traits::NamedMultiReservableCurrency;
use sp_runtime::traits::Zero;
use sp_runtime::{DispatchError, FixedU128};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

pub type BoundedRoute<AssetId> = BoundedVec<Trade<AssetId>, ConstU32<5>>;

#[derive(Clone, Encode, Decode, Eq, PartialEq, frame_support::PalletError, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum SolutionError {
	IncorrectIntentAmountResolution,
	IncorrectTransferInstruction,
}

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
		route: BoundedRoute<AssetId>,
	},
	SwapExactOut {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		amount_out: Balance,
		route: BoundedRoute<AssetId>,
	},
}

fn ensure_intent_resolution<T: Config>(
	intent: &Intent<T::AccountId, T::AssetId>,
	resolved_intent: &ResolvedIntent,
) -> bool {
	let amount_in = intent.swap.amount_in;
	let amount_out = intent.swap.amount_out;
	let resolved_in = resolved_intent.amount_in;
	let resolved_out = resolved_intent.amount_out;

	if amount_in == resolved_in {
		return resolved_out == amount_out;
	}

	if amount_out == resolved_out {
		return resolved_in == amount_in;
	}

	let realized = FixedU128::from_rational(resolved_out, resolved_in);
	let expected = FixedU128::from_rational(amount_out, amount_in);

	if realized < expected {
		return false;
	}
	let diff = realized - expected;
	diff <= FixedU128::from_rational(1, 1000)
}

pub struct ICEEngine<T, C, R>(sp_std::marker::PhantomData<(T, C, R)>);

impl<T: Config, C, R> ICEEngine<T, C, R>
where
	C: Mutate<T::AccountId, AssetId = T::AssetId, Balance = Balance>,
	R: RouterT<T::RuntimeOrigin, T::AssetId, Balance, Trade<T::AssetId>, AmountInAndOut<Balance>>,
{
	pub fn validate_solution(
		solution: &mut Solution<T::AccountId, T::AssetId>,
	) -> Result<Vec<(T::AssetId, Balance)>, DispatchError> {
		// Store resolved amounts for each account
		// This is used to ensure that the transfer instruction does not transfer more than it should
		let mut acc_amounts_in: BTreeMap<(T::AccountId, T::AssetId), Balance> = BTreeMap::new();
		let mut acc_amounts_out: BTreeMap<(T::AccountId, T::AssetId), Balance> = BTreeMap::new();

		let mut amounts_in: BTreeMap<T::AssetId, Balance> = BTreeMap::new();
		let mut amounts_out: BTreeMap<T::AssetId, Balance> = BTreeMap::new();

		// Check if resolved intents are valid:
		// - amounts not exceeding limit
		// - ensure ratio
		// - record resolved amount to check transfer instructions
		for resolved_intent in solution.intents.iter() {
			let intent =
				crate::Pallet::<T>::get_intent(resolved_intent.intent_id).ok_or(crate::Error::<T>::IntentNotFound)?;

			ensure!(
				ensure_intent_resolution::<T>(&intent, resolved_intent),
				crate::Error::<T>::InvalidSolution(SolutionError::IncorrectIntentAmountResolution)
			);

			let is_partial = intent.partial;
			let asset_in = intent.swap.asset_in;
			let asset_out = intent.swap.asset_out;

			let resolved_amount_in = resolved_intent.amount_in;
			let resolved_amount_out = resolved_intent.amount_out;

			amounts_in
				.entry(asset_in)
				.and_modify(|v| *v = v.saturating_add(resolved_amount_in))
				.or_insert(resolved_amount_in);
			amounts_out
				.entry(asset_out)
				.and_modify(|v| *v = v.saturating_add(resolved_amount_out))
				.or_insert(resolved_amount_out);

			match intent.swap.swap_type {
				SwapType::ExactIn => {
					if is_partial {
						ensure!(
							resolved_intent.amount_in <= intent.swap.amount_in,
							crate::Error::<T>::InvalidSolution(SolutionError::IncorrectIntentAmountResolution)
						);
					} else {
						ensure!(
							resolved_intent.amount_in == intent.swap.amount_in,
							crate::Error::<T>::InvalidSolution(SolutionError::IncorrectIntentAmountResolution)
						);
					}
					ensure!(
						resolved_intent.amount_out >= intent.swap.amount_out,
						crate::Error::<T>::InvalidSolution(SolutionError::IncorrectIntentAmountResolution)
					);

					acc_amounts_in
						.entry((intent.who.clone(), asset_in))
						.and_modify(|v| *v = v.saturating_add(resolved_intent.amount_in))
						.or_insert(resolved_intent.amount_in);
					acc_amounts_out
						.entry((intent.who.clone(), asset_out))
						.and_modify(|v| *v = v.saturating_add(resolved_intent.amount_out))
						.or_insert(resolved_intent.amount_out);
				}
				SwapType::ExactOut => {
					if is_partial {
						ensure!(
							resolved_intent.amount_out <= intent.swap.amount_out,
							crate::Error::<T>::InvalidSolution(SolutionError::IncorrectIntentAmountResolution)
						);
					} else {
						ensure!(
							resolved_intent.amount_out == intent.swap.amount_out,
							crate::Error::<T>::InvalidSolution(SolutionError::IncorrectIntentAmountResolution)
						);
					}
					ensure!(
						resolved_intent.amount_in <= intent.swap.amount_in,
						crate::Error::<T>::InvalidSolution(SolutionError::IncorrectIntentAmountResolution)
					);

					acc_amounts_in
						.entry((intent.who.clone(), asset_in))
						.and_modify(|v| *v = v.saturating_add(resolved_intent.amount_in))
						.or_insert(resolved_intent.amount_in);
					acc_amounts_out
						.entry((intent.who.clone(), asset_out))
						.and_modify(|v| *v = v.saturating_add(resolved_intent.amount_out))
						.or_insert(resolved_intent.amount_in);
				}
			}
		}

		// Validate instructions, correct transfer amounts, valid trades , calculate weight
		//TODO: needs to validate tht all the resolved intents are used in the instructions
		// meaning that transfer sends actual resolved amount. Because now, we ony checked the transfers,
		// but we dont check if all resolved amounts are transferred.
		for instruction in solution.instructions.iter() {
			match instruction {
				Instruction::TransferIn { who, asset_id, amount } => {
					ensure!(
						*acc_amounts_in.get(&(who.clone(), *asset_id)).unwrap_or(&0u128) == *amount,
						crate::Error::<T>::InvalidSolution(SolutionError::IncorrectTransferInstruction)
					);
				}
				Instruction::TransferOut { who, asset_id, amount } => {
					ensure!(
						*acc_amounts_out.get(&(who.clone(), *asset_id)).unwrap_or(&0u128) == *amount,
						crate::Error::<T>::InvalidSolution(SolutionError::IncorrectIntentAmountResolution)
					);
				}
				Instruction::SwapExactIn { .. } => {}
				Instruction::SwapExactOut { .. } => {}
			}
		}

		let mut matched_amounts = Vec::new();

		//TODO: we just checked the resolved amounts in and out, we should probably verify that the traded amounts is actually the matched amount?!
		for (asset_id, amount) in amounts_in.iter() {
			let amount_out = amounts_out.get(asset_id).unwrap_or(&0u128);
			matched_amounts.push((*asset_id, *(amount.min(amount_out))));
		}

		solution.weight = Self::calculate_weight(&solution.instructions)?;

		Ok(matched_amounts)
	}

	fn calculate_weight(
		_instructions: &BoundedInstructions<T::AccountId, T::AssetId>,
	) -> Result<Weight, DispatchError> {
		Ok(Weight::default())
	}

	fn update_intents(resolved_intents: BoundedResolvedIntents) -> DispatchResult {
		//TODO:
		// handle reserved amounts?? should be already handled in execution

		for resolved_intent in resolved_intents.iter() {
			let intent = Intents::<T>::take(&resolved_intent.intent_id).ok_or(crate::Error::<T>::IntentNotFound)?;

			let is_partial = intent.partial;
			let asset_in = intent.swap.asset_in;
			let asset_out = intent.swap.asset_out;

			let amount_in = intent.swap.amount_in;
			let amount_out = intent.swap.amount_out;

			let resolved_amount_in = resolved_intent.amount_in;
			let resolved_amount_out = resolved_intent.amount_out;

			let partially_resolved = resolved_amount_out != amount_out;

			if partially_resolved && !is_partial {
				return Err(Error::<T>::InvalidSolution(SolutionError::IncorrectTransferInstruction).into());
				//todo: better error
			}

			if partially_resolved {
				let new_intent = Intent {
					who: intent.who.clone(),
					swap: Swap {
						asset_in,
						asset_out,
						amount_in: amount_in.saturating_sub(resolved_amount_in),
						amount_out: amount_out.saturating_sub(resolved_amount_out),
						swap_type: intent.swap.swap_type,
					},
					deadline: intent.deadline,
					partial: true,
					on_success: intent.on_success,
					on_failure: intent.on_failure,
				};
				Intents::<T>::insert(resolved_intent.intent_id, new_intent);
			}
		}
		Ok(())
	}

	pub fn execute_solution(solution: Solution<T::AccountId, T::AssetId>) -> Result<(), DispatchError> {
		let holding_account = crate::Pallet::<T>::holding_account();

		for instruction in solution.instructions {
			match instruction {
				Instruction::TransferIn { who, asset_id, amount } => {
					let r = T::ReservableCurrency::unreserve_named(&T::NamedReserveId::get(), asset_id, &who, amount);
					ensure!(r == Balance::zero(), crate::Error::<T>::InsufficientReservedBalance);
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
						amount_out,
						amount_in, // set as limit in the instruction
						route.to_vec(),
					)?;
				}
			}
		}

		Self::update_intents(solution.intents)?;

		Ok(())
	}
}
