use crate::pallet::Intents;
use crate::types::{
	Balance, BoundedInstructions, BoundedResolvedIntents, BoundedTrades, Instruction, Intent, ResolvedIntent, Solution,
	Swap, SwapType, TradeInstructionTransform,
};
use crate::{Config, Error};
use frame_support::dispatch::DispatchResult;
use frame_support::ensure;
use frame_support::pallet_prelude::Get;
use frame_support::traits::fungibles::Mutate;
use frame_support::traits::tokens::Preservation;
use frame_support::traits::OriginTrait;
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::RouterT;
use orml_traits::NamedMultiReservableCurrency;
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::traits::{Convert, Zero};
use sp_runtime::{ArithmeticError, DispatchError, FixedU128, Rounding, Saturating};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

pub struct ICEEngine<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> ICEEngine<T> {
	pub fn prepare_solution(
		intents: BoundedResolvedIntents,
		trades: BoundedTrades<T::AssetId>,
		score: u64,
	) -> Result<Solution<T::AccountId, T::AssetId>, DispatchError> {
		// 1. Validate resolved intents - limit price
		// 2. Build list of transfers in and transfers out
		// 3. Merge with list of trades
		// 4. Calculate matched amount and score the solution
		// 5. Ensure score solution is correct
		// 6. How to validate trades ?! do we need ?

		let mut amounts_in: BTreeMap<T::AssetId, Balance> = BTreeMap::new();
		let mut amounts_out: BTreeMap<T::AssetId, Balance> = BTreeMap::new();

		let mut transfers_in: Vec<Instruction<T::AccountId, T::AssetId>> = Vec::new();
		let mut transfers_out: Vec<Instruction<T::AccountId, T::AssetId>> = Vec::new();

		for resolved_intent in intents.iter() {
			let intent = Intents::<T>::get(resolved_intent.intent_id).ok_or(Error::<T>::IntentNotFound)?;

			ensure!(
				ensure_intent_price::<T>(&intent, &resolved_intent),
				Error::<T>::IntentLimitPriceViolation
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

			transfers_in.push(Instruction::TransferIn {
				who: intent.who.clone(),
				asset_id: asset_in,
				amount: resolved_amount_in,
			});
			transfers_out.push(Instruction::TransferOut {
				who: intent.who.clone(),
				asset_id: asset_out,
				amount: resolved_amount_out,
			});

			match intent.swap.swap_type {
				SwapType::ExactIn => {
					if is_partial {
						ensure!(
							resolved_intent.amount_in <= intent.swap.amount_in,
							Error::<T>::IncorrectIntentAmountResolution
						);
					} else {
						ensure!(
							resolved_intent.amount_in == intent.swap.amount_in,
							Error::<T>::IncorrectIntentAmountResolution
						);
						ensure!(
							resolved_intent.amount_out >= intent.swap.amount_out,
							Error::<T>::IncorrectIntentAmountResolution
						);
					}
				}
				SwapType::ExactOut => {
					if is_partial {
						ensure!(
							resolved_intent.amount_out <= intent.swap.amount_out,
							Error::<T>::IncorrectIntentAmountResolution
						);
					} else {
						ensure!(
							resolved_intent.amount_out == intent.swap.amount_out,
							Error::<T>::IncorrectIntentAmountResolution
						);
						ensure!(
							resolved_intent.amount_in <= intent.swap.amount_in,
							Error::<T>::IncorrectIntentAmountResolution
						);
					}
				}
			}
		}

		let mut matched_amounts = Vec::new();
		for (asset_id, amount) in amounts_in.iter() {
			let amount_out = amounts_out.get(asset_id).unwrap_or(&0u128);
			matched_amounts.push((*asset_id, *(amount.min(amount_out))));
		}

		let mut instructions = Vec::new();

		instructions.extend(transfers_in.into_iter());
		instructions.extend(TradeInstructionTransform::convert(trades).into_iter());
		instructions.extend(transfers_out.into_iter());

		let solution = Solution {
			intents,
			instructions: BoundedInstructions::truncate_from(instructions),
		};

		let calculated_score = Self::score_solution(&solution, matched_amounts)?;

		ensure!(calculated_score == score, Error::<T>::InvalidScore);
		Ok(solution)
	}

	pub fn validate_solution(solution: &Solution<T::AccountId, T::AssetId>, score: u64) -> Result<(), DispatchError> {
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
			let intent = crate::Pallet::<T>::get_intent(resolved_intent.intent_id).ok_or(Error::<T>::IntentNotFound)?;

			ensure!(
				ensure_intent_price::<T>(&intent, resolved_intent),
				Error::<T>::IntentLimitPriceViolation
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
							Error::<T>::IncorrectIntentAmountResolution
						);
					} else {
						ensure!(
							resolved_intent.amount_in == intent.swap.amount_in,
							crate::Error::<T>::IncorrectIntentAmountResolution
						);
						ensure!(
							resolved_intent.amount_out >= intent.swap.amount_out,
							Error::<T>::IncorrectIntentAmountResolution
						);
					}

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
							Error::<T>::IncorrectIntentAmountResolution
						);
					} else {
						ensure!(
							resolved_intent.amount_out == intent.swap.amount_out,
							Error::<T>::IncorrectIntentAmountResolution
						);
						ensure!(
							resolved_intent.amount_in <= intent.swap.amount_in,
							Error::<T>::IncorrectIntentAmountResolution
						);
					}

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
						Error::<T>::InvalidTransferInstruction
					);
				}
				Instruction::TransferOut { who, asset_id, amount } => {
					ensure!(
						*acc_amounts_out.get(&(who.clone(), *asset_id)).unwrap_or(&0u128) == *amount,
						Error::<T>::InvalidTransferInstruction
					);
				}
				Instruction::SwapExactIn { .. } => {}
				Instruction::SwapExactOut { .. } => {}
			}
		}

		let mut matched_amounts = Vec::new();

		//TODO: we just checked the resolved amounts in and out, we should probably verify that the traded amounts is actually the difference?!
		for (asset_id, amount) in amounts_in.iter() {
			let amount_out = amounts_out.get(asset_id).unwrap_or(&0u128);
			matched_amounts.push((*asset_id, *(amount.min(amount_out))));
		}

		let calculated_score = Self::score_solution(&solution, matched_amounts)?;

		ensure!(calculated_score == score, Error::<T>::InvalidScore);

		Ok(())
	}

	fn score_solution(
		solution: &Solution<T::AccountId, T::AssetId>,
		matched_amounts: Vec<(T::AssetId, Balance)>,
	) -> Result<u64, DispatchError> {
		let resolved_intents = solution.intents.iter().count() as u128;

		let mut hub_amount = resolved_intents * 1_000_000_000_000u128;

		for (asset_id, amount) in matched_amounts {
			let price = T::PriceProvider::get_price(T::HubAssetId::get(), asset_id).ok_or(Error::<T>::MissingPrice)?;
			let converted = multiply_by_rational_with_rounding(amount, price.n, price.d, Rounding::Down)
				.ok_or(ArithmeticError::Overflow)?;
			hub_amount.saturating_accrue(converted);
		}

		// round down
		Ok((hub_amount / 1_000_000u128) as u64)
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

			// This should be handled by the validation, but just in case
			if partially_resolved && !is_partial {
				return Err(Error::<T>::IncorrectIntentAmountResolution.into());
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
					T::Currency::transfer(asset_id, &who, &holding_account, amount, Preservation::Expendable)?;
				}
				Instruction::TransferOut { who, asset_id, amount } => {
					T::Currency::transfer(asset_id, &holding_account, &who, amount, Preservation::Expendable)?;
				}
				Instruction::SwapExactIn {
					asset_in,
					asset_out,
					amount_in,
					amount_out,
					route,
				} => {
					let origin = T::RuntimeOrigin::signed(holding_account.clone().into());
					T::TradeExecutor::sell(
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
					T::TradeExecutor::buy(
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

fn ensure_intent_price<T: Config>(intent: &Intent<T::AccountId, T::AssetId>, resolved_intent: &ResolvedIntent) -> bool {
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
