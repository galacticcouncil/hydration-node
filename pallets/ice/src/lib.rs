// This file is part of https://github.com/galacticcouncil/*
//
//                $$$$$$$      Licensed under the Apache License, Version 2.0 (the "License")
//             $$$$$$$$$$$$$        you may only use this file in compliance with the License
//          $$$$$$$$$$$$$$$$$$$
//                      $$$$$$$$$       Copyright (C) 2021-2024  Intergalactic, Limited (GIB)
//         $$$$$$$$$$$   $$$$$$$$$$                       SPDX-License-Identifier: Apache-2.0
//      $$$$$$$$$$$$$$$$$$$$$$$$$$
//   $$$$$$$$$$$$$$$$$$$$$$$        $                      Built with <3 for decentralisation
//  $$$$$$$$$$$$$$$$$$$        $$$$$$$
//  $$$$$$$         $$$$$$$$$$$$$$$$$$      Unless required by applicable law or agreed to in
//   $       $$$$$$$$$$$$$$$$$$$$$$$       writing, software distributed under the License is
//      $$$$$$$$$$$$$$$$$$$$$$$$$$        distributed on an "AS IS" BASIS, WITHOUT WARRANTIES
//      $$$$$$$$$   $$$$$$$$$$$         OR CONDITIONS OF ANY KIND, either express or implied.
//        $$$$$$$$
//          $$$$$$$$$$$$$$$$$$            See the License for the specific language governing
//             $$$$$$$$$$$$$                   permissions and limitations under the License.
//                $$$$$$$
//                                                                 $$
//  $$$$$   $$$$$                    $$                       $
//   $$$     $$$  $$$     $$   $$$$$ $$  $$$ $$$$  $$$$$$$  $$$$  $$$    $$$$$$   $$ $$$$$$
//   $$$     $$$   $$$   $$  $$$    $$$   $$$  $  $$     $$  $$    $$  $$     $$   $$$   $$$
//   $$$$$$$$$$$    $$  $$   $$$     $$   $$        $$$$$$$  $$    $$  $$     $$$  $$     $$
//   $$$     $$$     $$$$    $$$     $$   $$     $$$     $$  $$    $$   $$     $$  $$     $$
//  $$$$$   $$$$$     $$      $$$$$$$$ $ $$$      $$$$$$$$   $$$  $$$$   $$$$$$$  $$$$   $$$$
//                  $$$

#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

pub mod api;
mod traits;
pub mod types;
mod weights;

use crate::traits::AMMState;
use frame_benchmarking::v2::__private::log;
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::*;
use frame_support::traits::Get;
use frame_support::PalletId;
use frame_system::offchain::SendTransactionTypes;
use frame_system::pallet_prelude::*;
use frame_system::Origin;
use hydra_dx_math::types::Ratio;
use orml_traits::MultiCurrency;
use pallet_intent::types::AssetId;
use pallet_intent::types::Intent;
use pallet_intent::types::IntentId;
use sp_core::U512;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::traits::BlockNumberProvider;
use sp_runtime::traits::CheckedConversion;
use sp_runtime::traits::One;
use sp_runtime::traits::Saturating;
use sp_runtime::traits::Zero;
use std::collections::{HashMap, HashSet};

pub use pallet::*;
use types::*;
pub use weights::WeightInfo;

//TODO: make sure tx is always first in the block(same as liquidations), this is tmp
pub const UNSIGNED_TXS_PRIORITY: u64 = u64::max_value();

const OCW_LOG_TARGET: &str = "ice::offchain_worker";
pub(crate) const OCW_TAG_PREFIX: &str = "ice-solution";
pub(crate) const OCW_PROVIDES: &[u8; 15] = b"submit_solution";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_benchmarking::__private::log;
	use frame_system::offchain::SubmitTransaction;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ pallet_intent::Config
		+ pallet_route_executor::Config<AssetId = AssetId>
		+ SendTransactionTypes<Call<Self>>
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Multi currency mechanism
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = AssetId, Balance = Balance>;

		/// Pallet id - used to create a holding account
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		/// AMM state provider trait - returns opaque state for solver
		type AMM: traits::AMMState;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Solution has been executed.
		SolutionExecuted {
			//NOTE: do we need block number? solution is executed in the block when event was triggered
			intents_executed: u64,
			trades_executed: u64,
			score: Score,
		},

		IntentResolved {
			intent_id: IntentId,
			owner: T::AccountId,
			asset_in: AssetId,
			asset_out: AssetId,
			amount_in: Balance,
			amount_out: Balance,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Provided solution is not valid.
		InvalidSolution,
		/// Solution target doesn't match current block.
		InvalidTargetBlock,
		/// Referenced intent doesn't exist.
		IntentNotFound,
		/// Referenced intent's owner doesn't exist.
		IntentOwnerNotFound,
		/// Referenced intent has expired.
		IntentExpired,
		/// Resolution violates user's limit.
		LimitViolation,
		///  Total inputs don't equal total outputs for some asset.
		BalanceImbalance,
		///  Trade price doesn't match clearing price.
		PriceInconsistency,
		/// Asset involved in trade has no clearing price defined.
		MissingClearingPrice,
		/// Same intent referenced multiple times.
		DuplicateIntent,
		/// Same asset has multiple clearing prices.
		DuplicateClearingPrice,
		/// Price ratio has zero denominator or numerator.
		InvalidPriceRatio,
		/// Trade route is invalid.
		InvalidRoute,
		/// Claimed score doesn't match calculated score.
		ScoreMismatch,
		/// Intent's kind is not supported.
		UnsupportedIntentKind,
		/// Caluclation overflow.
		ArithmeticOverflow,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::submit_solution())]
		pub fn submit_solution(
			origin: OriginFor<T>,
			solution: Solution,
			score: Score,
			valid_for_block: BlockNumberFor<T>,
		) -> DispatchResult {
			ensure_none(origin)?;

			ensure!(
				valid_for_block == T::BlockNumberProvider::current_block_number(),
				Error::<T>::InvalidTargetBlock
			);

			ensure!(
				!solution.resolved.is_empty() && !solution.trades.is_empty(),
				Error::<T>::InvalidSolution
			);

			let mut clearing_prices: HashMap<AssetId, Ratio> = HashMap::with_capacity(solution.clearing_prices.len());
			Self::validate_clearing_prices(&mut clearing_prices, &solution.clearing_prices)?;

			let mut processed_intents: HashSet<IntentId> = HashSet::with_capacity(solution.resolved.len());
			let holding_pot = Self::get_pallet_account();
			let holding_origin: OriginFor<T> = Origin::<T>::Signed(holding_pot.clone()).into();

			// TODO: this is not most prerformant solution, verify it works and optimise

			for (id, intent) in &solution.resolved {
				let owner = pallet_intent::Pallet::<T>::intent_owner(id).ok_or(Error::<T>::IntentOwnerNotFound)?;
				pallet_intent::Pallet::<T>::unlock_funds(*id, intent.amount_in())?;

				<T as Config>::Currency::transfer(intent.asset_in(), &owner, &holding_pot, intent.amount_in())?;
			}

			for t in &solution.trades {
				match t.trade_type {
					TradeType::Buy => {
						pallet_route_executor::Pallet::<T>::buy(
							holding_origin.clone(),
							t.route.first().ok_or(Error::<T>::InvalidRoute)?.asset_in,
							t.route.last().ok_or(Error::<T>::InvalidRoute)?.asset_out,
							t.amount_out.into(),
							t.amount_in.into(),
							t.route.clone(),
						)?;
					}
					TradeType::Sell => {
						pallet_route_executor::Pallet::<T>::sell(
							holding_origin.clone(),
							t.route.first().ok_or(Error::<T>::InvalidRoute)?.asset_in,
							t.route.last().ok_or(Error::<T>::InvalidRoute)?.asset_out,
							t.amount_in.into(),
							t.amount_out.into(),
							t.route.clone(),
						)?;
					}
				}
			}

			let mut exec_score: Score = 0;
			for (id, resolved) in &solution.resolved {
				ensure!(processed_intents.insert(*id), Error::<T>::DuplicateIntent);

				let owner = pallet_intent::Pallet::<T>::intent_owner(id).ok_or(Error::<T>::IntentOwnerNotFound)?;

				<T as Config>::Currency::transfer(resolved.asset_out(), &holding_pot, &owner, resolved.amount_out())?;

				Self::validate_price_consitency(&clearing_prices, resolved)?;

				Self::deposit_event(Event::IntentResolved {
					intent_id: *id,
					owner: owner.clone(),
					asset_in: resolved.asset_in(),
					asset_out: resolved.asset_out(),
					amount_in: resolved.amount_in(),
					amount_out: resolved.amount_out(),
				});

				let intent = pallet_intent::Pallet::<T>::get_intent(id).ok_or(Error::<T>::IntentNotFound)?;
				let s = intent.surplus(&resolved).ok_or(Error::<T>::ArithmeticOverflow)?;
				exec_score = exec_score.checked_add(s).ok_or(Error::<T>::ArithmeticOverflow)?;

				pallet_intent::Pallet::<T>::intent_resolved(*id, &owner, &resolved)?;
			}

			ensure!(score == exec_score, Error::<T>::ScoreMismatch);

			Self::deposit_event(Event::SolutionExecuted {
				intents_executed: solution.resolved.len() as u64,
				trades_executed: solution.trades.len() as u64,
				score,
			});

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(_n: BlockNumberFor<T>) {}

		fn offchain_worker(block_number: BlockNumberFor<T>) {
			let Some(call) = Self::run(block_number, |i, d| api::ice::get_solution(i, d)) else {
				//No call/solution, nothing to do
				return;
			};

			if let Err(e) = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()) {
				log::error!(target: OCW_LOG_TARGET, "submit solution, err: {:?}", e);
			};
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		/// Validates unsigned transactions for solution execution
		///
		/// This function ensures that only valid solution transactions originating from
		/// offchain workers are accepted, and prevents unauthorized external calls.
		fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			match source {
				TransactionSource::Local | TransactionSource::InBlock => { /*OCW or included in block are allowed */ }
				_ => {
					return InvalidTransaction::Call.into();
				}
			};

			let block_no = T::BlockNumberProvider::current_block_number();
			if let Call::submit_solution {
				solution,
				score,
				valid_for_block,
			} = call
			{
				if !valid_for_block.eq(&block_no.saturating_add(One::one())) {
					log::error!(target: OCW_LOG_TARGET, "invalid target block,  target_block: {:?}, block: {:?}", valid_for_block, block_no);
					return InvalidTransaction::Call.into();
				}

				let exec_score = match Self::validate_unsigned_solution(&solution) {
					Ok(ec) => ec,
					Err(e) => {
						log::error!(target: OCW_LOG_TARGET, "validate solution, err: {:?}, block: {:?}", e, block_no);
						return InvalidTransaction::Call.into();
					}
				};

				if exec_score != *score {
					log::error!(target: OCW_LOG_TARGET, "score mismatch,  score: {:?}, exec_score: {:?}, block: {:?}", score, exec_score, block_no);
					return InvalidTransaction::Call.into();
				}

				return ValidTransaction::with_tag_prefix(OCW_TAG_PREFIX)
					.priority(UNSIGNED_TXS_PRIORITY)
					.and_provides(OCW_PROVIDES.to_vec())
					.longevity(1)
					.propagate(false)
					.build();
			}

			InvalidTransaction::Call.into()
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn get_pallet_account() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	/// Function validtes if intent was resolved based on clearing price.
	fn validate_price_consitency(
		clearing_prices: &HashMap<AssetId, Ratio>,
		resolved: &Intent,
	) -> Result<(), DispatchError> {
		let cp_in = clearing_prices
			.get(&resolved.asset_in())
			.ok_or(Error::<T>::MissingClearingPrice)?;
		let cp_out = clearing_prices
			.get(&resolved.asset_out())
			.ok_or(Error::<T>::MissingClearingPrice)?;

		ensure!(
			Self::calc_amount_out(resolved.amount_in(), cp_in, cp_out)
				.ok_or(Error::<T>::ArithmeticOverflow)?
				.eq(&resolved.amount_out()),
			Error::<T>::PriceInconsistency
		);

		Ok(())
	}

	/// Function validates values of `clearing_prices` and adds it into `valid_prices`.
	fn validate_clearing_prices(
		valid_prices: &mut HashMap<AssetId, Ratio>,
		clearing_prices: &Vec<(AssetId, Ratio)>,
	) -> Result<(), DispatchError> {
		for cp in clearing_prices {
			ensure!(!cp.1.n.is_zero() && !cp.1.d.is_zero(), Error::<T>::InvalidPriceRatio);
			ensure!(
				valid_prices.insert(cp.0, cp.1).is_none(),
				Error::<T>::DuplicateClearingPrice
			);
		}

		Ok(())
	}

	/// Function calculates amount out based on asset in and asset out prices denominated in common asset.
	/// ```ignore
	/// rate = price_in / price_out
	///		= (num_in / denom_in) / (num_out / denom_out)
	///		= (num_in × denom_out) / (denom_in × num_out)
	///	```
	/// ```ignore
	/// out = amount_in × rate
	///		= amount_in × (num_in × denom_out) / (denom_in × num_out)
	///	```
	fn calc_amount_out(amount_in: Balance, price_in: &Ratio, price_out: &Ratio) -> Option<u128> {
		let n = U512::from(price_in.n).checked_mul(U512::from(price_out.d))?;
		let d = U512::from(price_in.d).checked_mul(U512::from(price_out.n))?;

		n.checked_mul(U512::from(amount_in))?.checked_div(d)?.checked_into()
	}

	/// Function validates provided solution and returns solution's score if solution is
	/// valid.
	fn validate_unsigned_solution(s: &Solution) -> Result<Score, DispatchError> {
		//TODO:
		// * add weight rule and make sure sollution respets it.

		let mut clearing_prices: HashMap<AssetId, Ratio> = HashMap::with_capacity(s.clearing_prices.len());
		Self::validate_clearing_prices(&mut clearing_prices, &s.clearing_prices)?;

		let mut processed_intents: HashSet<IntentId> = HashSet::with_capacity(s.resolved.len());
		let mut score: Score = 0;
		for (id, resolved) in &s.resolved {
			let intent = pallet_intent::Pallet::<T>::get_intent(id).ok_or(Error::<T>::IntentNotFound)?;

			let s = intent.surplus(resolved).ok_or(Error::<T>::ArithmeticOverflow)?;
			score = score.checked_add(s).ok_or(Error::<T>::ArithmeticOverflow)?;

			ensure!(processed_intents.insert(*id), Error::<T>::DuplicateIntent);

			pallet_intent::Pallet::<T>::validate_resolved(*id, resolved)?;

			Self::validate_price_consitency(&clearing_prices, resolved)?;
		}

		Ok(score)
	}

	pub fn run<F>(block_no: BlockNumberFor<T>, solve: F) -> Option<Call<T>>
	where
		F: FnOnce(Vec<u8>, Vec<u8>) -> Option<Vec<u8>>,
	{
		let intents = pallet_intent::Pallet::<T>::get_valid_intents();
		let state = <T as Config>::AMM::get_state();

		let Some(s) = solve(intents.encode(), state.encode()) else {
			log::debug!(target: OCW_LOG_TARGET, "no solution found, block: {:?}", block_no);
			return None;
		};

		let solution = match Solution::decode(&mut s.as_slice()) {
			Ok(s) => s,
			Err(e) => {
				log::error!(target: OCW_LOG_TARGET, "to decode solver's solution, err: {:?}, block: {:?}", e, block_no);
				return None;
			}
		};

		match Self::validate_unsigned_solution(&solution) {
			Ok(score) => Some(Call::submit_solution {
				solution,
				score,
				valid_for_block: block_no.saturating_add(One::one()),
			}),
			Err(e) => {
				log::error!(target: OCW_LOG_TARGET, "validate solution, err: {:?}, block: {:?}", e, block_no);
				None
			}
		}
	}
}
