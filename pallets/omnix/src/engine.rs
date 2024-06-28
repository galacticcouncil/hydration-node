use crate::pallet::Intents;
use crate::types::{Balance, Price, Solution, SwapType};
use crate::Config;
use frame_support::dispatch::RawOrigin;
use frame_support::ensure;
use frame_support::pallet_prelude::Get;
use frame_support::traits::fungibles::Mutate;
use frame_support::traits::tokens::Preservation;
use hydradx_traits::router::RouterT;
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::{DispatchError, Rounding};
use sp_std::collections::btree_map::BTreeMap;

#[derive(Debug, PartialEq)]
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
	HubSwap {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		amount_out: Balance,
	},
}

#[derive(Debug, PartialEq)]
pub struct Plan<AccountId, AssetId> {
	pub instructions: Vec<Instruction<AccountId, AssetId>>,
}

pub struct OmniXEngine<T, C, R>(std::marker::PhantomData<(T, C, R)>);

impl<T: Config, C, R> OmniXEngine<T, C, R>
where
	C: Mutate<T::AccountId, AssetId = T::AssetId, Balance = Balance>,
	R: RouterT<
		T::RuntimeOrigin,
		T::AssetId,
		Balance,
		hydradx_traits::router::Trade<T::AssetId>,
		hydradx_traits::router::AmountInAndOut<Balance>,
	>,
{
	fn calculate_transfer_amounts(
		solution: &Solution<T::AccountId, T::AssetId>,
	) -> Result<Vec<Instruction<T::AccountId, T::AssetId>>, DispatchError> {
		// Result will be:
		// list of transfers in
		// list of lrna swaps
		// list of transfers out
		let mut transfers_in = Vec::new();
		let mut transfers_out = Vec::new();
		let mut hub_asset_swaps: Vec<Instruction<T::AccountId, T::AssetId>> = Vec::new();
		let mut deltas_in: BTreeMap<T::AssetId, Balance> = BTreeMap::new();
		let mut deltas_out: BTreeMap<T::AssetId, Balance> = BTreeMap::new();

		let sell_prices: BTreeMap<T::AssetId, (Balance, Balance)> = solution.sell_prices.clone().into_iter().collect();
		let buy_prices: BTreeMap<T::AssetId, (Balance, Balance)> = solution.buy_prices.clone().into_iter().collect();

		for intent in solution.intents.iter() {
			let intent_id = intent.intent_id;
			let amount = intent.amount;

			let intent = Intents::<T>::get(intent_id).ok_or(crate::pallet::Error::<T>::IntentNotFound)?;

			match intent.swap.swap_type {
				SwapType::ExactInput => {
					// Calculate amount out
					// STore deltas
					// Ensure limits
					// amount out = amount_in * sell price / buy price
					let sell_price = sell_prices
						.get(&intent.swap.asset_in)
						.ok_or(crate::pallet::Error::<T>::MissingPrice)?;
					let buy_price = buy_prices
						.get(&intent.swap.asset_out)
						.ok_or(crate::pallet::Error::<T>::MissingPrice)?;
					let amount_out = calculate_out_amount(amount, *sell_price, *buy_price)
						.ok_or(crate::pallet::Error::<T>::InvalidSolution)?;
					let transfer = Instruction::TransferIn {
						who: intent.who.clone(),
						asset_id: intent.swap.asset_in,
						amount,
					};
					transfers_in.push(transfer);

					let transfer = Instruction::TransferOut {
						who: intent.who,
						asset_id: intent.swap.asset_out,
						amount: amount_out,
					};
					transfers_out.push(transfer);

					deltas_in
						.entry(intent.swap.asset_in)
						.and_modify(|e| *e = e.saturating_add(amount))
						.or_insert(amount);
					deltas_out
						.entry(intent.swap.asset_out)
						.and_modify(|e| *e = e.saturating_add(amount_out))
						.or_insert(amount_out);
				}
				SwapType::ExactOutput => {
					// Calculate amount in
					// Store deltas
					// Ensure limits
					let amount_in = amount; //TODO calculate
						// TODO: ensure intent limit
					let transfer = Instruction::TransferIn {
						who: intent.who.clone(),
						asset_id: intent.swap.asset_in,
						amount: amount_in,
					};
					transfers_in.push(transfer);

					let transfer = Instruction::TransferOut {
						who: intent.who,
						asset_id: intent.swap.asset_out,
						amount,
					};

					transfers_out.push(transfer);

					deltas_in
						.entry(intent.swap.asset_in)
						.and_modify(|e| *e = e.saturating_add(amount_in))
						.or_insert(amount_in);
					deltas_out
						.entry(intent.swap.asset_out)
						.and_modify(|e| *e = e.saturating_add(amount))
						.or_insert(amount);
				}
			}
		}

		// Calculate deltas and how much needs to be swapped
		// First sell for lrna
		for (asset_id, delta_in) in deltas_in.iter() {
			let delta_out = deltas_out.get(asset_id).unwrap_or(&0);
			if delta_in > delta_out {
				let swap = Instruction::HubSwap {
					asset_in: *asset_id,
					asset_out: T::HubAssetId::get(),
					amount_in: delta_in - delta_out,
					amount_out: 0, //TODO limit?
				};
				hub_asset_swaps.push(swap);
			}
		}

		// Now buys for lrna
		for (asset_id, delta_out) in deltas_out.iter() {
			let delta_in = deltas_in.get(asset_id).unwrap_or(&0);
			if delta_out > delta_in {
				let swap = Instruction::<T::AccountId, T::AssetId>::HubSwap {
					asset_in: T::HubAssetId::get(),
					asset_out: *asset_id,
					amount_in: Balance::MAX, //TODO limit?
					amount_out: delta_out - delta_in,
				};
				hub_asset_swaps.push(swap);
			}
		}

		let mut result = Vec::new();
		result.extend(transfers_in);
		result.extend(hub_asset_swaps);
		result.extend(transfers_out);

		Ok(result)
	}

	fn validate_transfers(_transfers: &[Instruction<T::AccountId, T::AssetId>]) -> Result<(), DispatchError> {
		//TODO: check balances
		Ok(())
	}

	pub fn prepare_solution(
		solution: &Solution<T::AccountId, T::AssetId>,
	) -> Result<Plan<T::AccountId, T::AssetId>, DispatchError> {
		ensure!(
			solution.sell_prices.len() == solution.buy_prices.len(),
			crate::pallet::Error::<T>::InvalidSolution
		);

		let instructions = Self::calculate_transfer_amounts(&solution)?;

		Self::validate_transfers(&instructions)?;
		//TODO: weights?

		let plan = Plan { instructions };

		Ok(plan)
	}

	pub fn execute_solution(solution: Plan<T::AccountId, T::AssetId>) -> Result<(), DispatchError> {
		let holding_account = crate::Pallet::<T>::holding_account();
		for instruction in solution.instructions {
			match instruction {
				Instruction::TransferIn { who, asset_id, amount } => {
					C::transfer(asset_id, &who, &holding_account, amount, Preservation::Expendable)?;
				}
				Instruction::TransferOut { who, asset_id, amount } => {
					C::transfer(asset_id, &holding_account, &who, amount, Preservation::Expendable)?;
				}
				Instruction::HubSwap {
					asset_in,
					asset_out,
					amount_in,
					amount_out,
				} => {
					debug_assert!(
						asset_in == T::HubAssetId::get() || asset_out == T::HubAssetId::get(),
						"No Hub asset in the trade"
					);
					if asset_in == T::HubAssetId::get() {
						// buy token
						R::buy(
							RawOrigin::Signed(holding_account.clone().into()).into(),
							asset_in,
							asset_out,
							amount_out,
							amount_in, // it is set as limit in the instruction
							vec![],
						)?;
					} else {
						// sell token
						R::sell(
							RawOrigin::Signed(holding_account.clone().into()).into(),
							asset_in,
							asset_out,
							amount_in,
							amount_out, // set as limit in the instruction
							vec![],
						)?;
					}
				}
			}
		}
		Ok(())
	}
}

fn calculate_out_amount(amount_in: Balance, sell_price: Price, buy_price: Price) -> Option<Balance> {
	//TODO: Verify calculate, rounding? or other way to calculate to minimize rounding errors
	let amt = multiply_by_rational_with_rounding(amount_in, sell_price.0, sell_price.1, Rounding::Down)?;
	multiply_by_rational_with_rounding(amt, buy_price.1, buy_price.0, Rounding::Down)
}
