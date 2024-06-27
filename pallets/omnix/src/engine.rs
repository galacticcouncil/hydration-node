use crate::pallet::Intents;
use crate::types::{Balance, IntentId, Price, ResolvedIntent, Solution, SwapType};
use crate::Config;
use frame_support::ensure;
use frame_support::pallet_prelude::Get;
use frame_support::traits::tokens::AssetId;
use sp_runtime::DispatchError;
use sp_std::collections::btree_map::BTreeMap;

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

pub struct Plan<AccountId, AssetId> {
	pub instructions: Vec<Instruction<AccountId, AssetId>>,
}

pub struct OmniXEngine<T>(std::marker::PhantomData<T>);

impl<T: Config> OmniXEngine<T> {
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

		for intent in solution.intents.iter() {
			let intent_id = intent.intent_id;
			let amount = intent.amount;

			let intent = Intents::<T>::get(intent_id).ok_or(crate::pallet::Error::<T>::IntentNotFound)?;

			match intent.swap.swap_type {
				SwapType::ExactInput => {
					// Calculate amount out
					// STore deltas
					// Ensure limits
					let amount_out = 1; //TODO calculate
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
						.or_insert(amount);
				}
				SwapType::ExactOutput => {
					// Calculate amount in
					// Store deltas
					// Ensure limits
					let amount_in = amount; //TODO calculate
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
						.or_insert(amount);
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
		for (asset_id, delta_in) in deltas_in.iter() {
			let delta_out = deltas_out.get(asset_id).unwrap_or(&0);
			if delta_out > delta_in {
				let swap = Instruction::HubSwap {
					asset_in: T::HubAssetId::get(),
					asset_out: *asset_id,
					amount_in: Balance::MAX, //TODO limit?
					amount_out: delta_out - delta_in,
				};
			}
		}

		let mut result = Vec::new();
		result.extend(transfers_in);
		result.extend(hub_asset_swaps);
		result.extend(transfers_out);

		Ok(result)
	}

	fn validate_transfers(transfers: &[Instruction<T::AccountId, T::AssetId>]) -> Result<(), DispatchError> {
		todo!()
	}

	pub fn prepare_solution(
		solution: &Solution<T::AccountId, T::AssetId>,
	) -> Result<Plan<T::AccountId, T::AssetId>, DispatchError> {
		ensure!(
			solution.sell_prices.len() == solution.buy_prices.len(),
			crate::pallet::Error::<T>::InvalidSolution
		);

		let transfers = Self::calculate_transfer_amounts(&solution)?;

		Self::validate_transfers(&transfers)?;

		todo!()
	}

	pub fn execute_solution(solution: Plan<T::AccountId, T::AssetId>) -> Result<(), DispatchError> {
		todo!()
	}
}
