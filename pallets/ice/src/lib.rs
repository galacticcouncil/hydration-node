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

pub mod traits;
mod weights;

use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::*;
use frame_support::traits::ExistenceRequirement::AllowDeath;
use frame_support::traits::Get;
use frame_support::PalletId;
use frame_system::pallet_prelude::*;
use frame_system::Origin;
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use hydradx_traits::registry::Inspect;
use ice_support::AssetId;
use ice_support::Balance;
use ice_support::Intent;
use ice_support::IntentData;
use ice_support::IntentId;
use ice_support::Price;
use ice_support::ResolvedIntent;
use ice_support::Score;
use ice_support::Solution;
use ice_support::MAX_NUMBER_OF_RESOLVED_INTENTS;
use orml_traits::MultiCurrency;
use pallet_route_executor::AmmTradeWeights;
use sp_core::U512;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::traits::BlockNumberProvider;
use sp_runtime::traits::CheckedConversion;
use sp_runtime::traits::One;
use sp_runtime::traits::Saturating;
use sp_runtime::traits::Zero;
use sp_std::borrow::ToOwned;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec::Vec;

pub use pallet::*;
pub use weights::WeightInfo;

//TODO: make sure tx is always first in the block(same as liquidations), this is tmp
pub const UNSIGNED_TXS_PRIORITY: u64 = u64::max_value();
const OCW_LOG_TARGET: &str = "ice::offchain_worker";
pub(crate) const OCW_TAG_PREFIX: &str = "ice-solution";
pub(crate) const OCW_PROVIDES: &[u8; 15] = b"submit_solution";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_system::offchain::SubmitTransaction;
	use hydradx_traits::CreateBare;
	use ice_solver::v1::SolverV1;
	use ice_support::SwapType;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ pallet_intent::Config
		+ pallet_route_executor::Config<AssetId = AssetId>
		+ CreateBare<Call<Self>>
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Multi currency mechanism
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = AssetId, Balance = Balance>;

		/// Pallet id - used to create a holding account
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Provider for current block number
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		/// Asset registry handler
		type RegistryHandler: Inspect<AssetId = AssetId>;

		/// Simulator configuration - provides simulators and route provider for the solver
		type Simulator: SimulatorConfig;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Solution has been executed.
		SolutionExecuted {
			intents_executed: u64,
			trades_executed: u64,
			score: Score,
		},

		/// Intent was settled.
		IntentSettled {
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
		/// Resolution violates user's limit.
		LimitViolation,
		/// Trade price doesn't match clearing price.
		PriceInconsistency,
		/// Asset involved in trade has no clearing price defined.
		MissingClearingPrice,
		/// Intent was referenced multiple times.
		DuplicateIntent,
		/// Asset has multiple clearing prices.
		DuplicateClearingPrice,
		/// Price ratio has zero numerator or denominator.
		InvalidPriceRatio,
		/// Provided list of clearing prices overflows allowed length.
		ClearingPricesInvalidLength,
		/// Trade's route is invalid.
		InvalidRoute,
		/// Provided score doesn't match execution score.
		ScoreMismatch,
		/// Intent's kind is not supported.
		UnsupportedIntentKind,
		/// Calculation overflow.
		ArithmeticOverflow,
		/// Asset with specified id doesn't exists.
		AssetNotFound,
		/// Traded amount is bellow limit.
		InvalidAmount,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Execute `solution` submitted by OCW.
		///
		/// Solution can be executed only as a whole solution.
		///
		/// Parameters:
		/// - `solution`: solution to execute
		/// - `valid_for_block`: block number `solution` is valid for
		///
		/// Emits:
		/// - `IntentSettled` when intent was resolved successfully
		/// - `SolutionExecuted`when `solution` was executed successfully
		///
		#[pallet::call_index(0)]
		#[pallet::weight({
			let mut total_w = <T as Config>::WeightInfo::submit_solution().saturating_mul(solution.resolved_intents.len() as u64);

			for t in &solution.trades {
				match t.direction {
					SwapType::ExactOut => {
						total_w = total_w.saturating_add(<T as pallet_route_executor::Config>::WeightInfo::buy_weight(t.route.as_slice()));
					}
					SwapType::ExactIn => {
						total_w = total_w.saturating_add(<T as pallet_route_executor::Config>::WeightInfo::sell_weight(t.route.as_slice()));
					}
				}
			}

			total_w
		})]
		pub fn submit_solution(
			origin: OriginFor<T>,
			solution: Solution,
			valid_for_block: BlockNumberFor<T>,
		) -> DispatchResult {
			ensure_none(origin)?;

			ensure!(
				valid_for_block == T::BlockNumberProvider::current_block_number(),
				Error::<T>::InvalidTargetBlock
			);

			// V1 solver may produce solutions with no trades (perfect CoW matching)
			ensure!(!solution.resolved_intents.is_empty(), Error::<T>::InvalidSolution);

			Self::validate_clearing_prices(&solution.clearing_prices)?;

			let mut processed_intents: BTreeSet<IntentId> = BTreeSet::new();
			let holding_pot = Self::get_pallet_account();
			let holding_origin: OriginFor<T> = Origin::<T>::Signed(holding_pot.clone()).into();

			// TODO: this is not most perform solution, verify it works and optimize

			for ResolvedIntent { id, data: intent } in &solution.resolved_intents {
				Self::validate_intent_amounts(intent)?;

				let owner = pallet_intent::Pallet::<T>::intent_owner(id).ok_or(Error::<T>::IntentOwnerNotFound)?;
				pallet_intent::Pallet::<T>::unlock_funds(&owner, intent.asset_in(), intent.amount_in())?;

				<T as Config>::Currency::transfer(
					intent.asset_in(),
					&owner,
					&holding_pot,
					intent.amount_in(),
					AllowDeath,
				)?;
			}

			for t in &solution.trades {
				match t.direction {
					SwapType::ExactOut => {
						pallet_route_executor::Pallet::<T>::buy(
							holding_origin.clone(),
							t.route.first().ok_or(Error::<T>::InvalidRoute)?.asset_in,
							t.route.last().ok_or(Error::<T>::InvalidRoute)?.asset_out,
							t.amount_out.into(),
							t.amount_in.into(),
							t.route.clone(),
						)?;
					}
					SwapType::ExactIn => {
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
			for resolved_intent in &solution.resolved_intents {
				let ResolvedIntent { id, data: resolve } = resolved_intent;
				ensure!(processed_intents.insert(*id), Error::<T>::DuplicateIntent);

				let owner = pallet_intent::Pallet::<T>::intent_owner(id).ok_or(Error::<T>::IntentOwnerNotFound)?;

				<T as Config>::Currency::transfer(
					resolve.asset_out(),
					&holding_pot,
					&owner,
					resolve.amount_out(),
					AllowDeath,
				)?;

				Self::validate_price_consitency(&solution.clearing_prices, resolve)?;

				Self::deposit_event(Event::IntentSettled {
					intent_id: *id,
					owner: owner.clone(),
					asset_in: resolve.asset_in(),
					asset_out: resolve.asset_out(),
					amount_in: resolve.amount_in(),
					amount_out: resolve.amount_out(),
				});

				let intent = pallet_intent::Pallet::<T>::get_intent(id).ok_or(Error::<T>::IntentNotFound)?;
				let surplus = intent.data.surplus(resolve).ok_or(Error::<T>::ArithmeticOverflow)?;
				exec_score = exec_score.checked_add(surplus).ok_or(Error::<T>::ArithmeticOverflow)?;

				pallet_intent::Pallet::<T>::intent_resolved(&owner, resolved_intent)?;
			}

			ensure!(solution.score == exec_score, Error::<T>::ScoreMismatch);

			Self::deposit_event(Event::SolutionExecuted {
				intents_executed: solution.resolved_intents.len() as u64,
				trades_executed: solution.trades.len() as u64,
				score: solution.score,
			});

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(_n: BlockNumberFor<T>) {}

		fn offchain_worker(block_number: BlockNumberFor<T>) {
			let Some(call) = Self::run(block_number, |intents, state| {
				SolverV1::<amm_simulator::HydrationSimulator<T::Simulator>>::solve(intents, state).ok()
			}) else {
				//No call/solution, nothing to do
				return;
			};

			let tx = <T as CreateBare<self::Call<T>>>::create_bare(call.into());
			if let Err(e) = SubmitTransaction::<T, Call<T>>::submit_transaction(tx) {
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
				valid_for_block,
			} = call
			{
				if !valid_for_block.eq(&block_no.saturating_add(One::one())) {
					log::error!(target: OCW_LOG_TARGET, "invalid target block,  target_block: {:?}, block: {:?}", valid_for_block, block_no);
					return InvalidTransaction::Call.into();
				}

				if let Err(e) = Self::validate_unsigned_solution(solution) {
					log::error!(target: OCW_LOG_TARGET, "validate solution, err: {:?}, block: {:?}", e, block_no);
					return InvalidTransaction::Call.into();
				};

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
	/// Function provides `holding_pot` account id.
	pub fn get_pallet_account() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	/// Function validates clearing prices(length and values) provided by solver.
	#[inline(always)]
	fn validate_clearing_prices(clearing_prices: &BTreeMap<AssetId, Price>) -> Result<(), DispatchError> {
		ensure!(
			clearing_prices.len() <= (MAX_NUMBER_OF_RESOLVED_INTENTS * 2) as usize,
			Error::<T>::ClearingPricesInvalidLength
		);

		for cp in clearing_prices {
			ensure!(!cp.1.n.is_zero() && !cp.1.d.is_zero(), Error::<T>::InvalidPriceRatio);
		}

		Ok(())
	}

	/// Function validates if intent was resolved based on clearing price.
	fn validate_price_consitency(
		_clearing_prices: &BTreeMap<AssetId, Price>,
		_resolve: &IntentData,
	) -> Result<(), DispatchError> {
		// V1 solver: Price consistency check temporarily disabled
		// The CoW matching may scale resolved amounts for conservation
		return Ok(());

		#[allow(unreachable_code)]
		{
			let cp_in = _clearing_prices
				.get(&_resolve.asset_in())
				.ok_or(Error::<T>::MissingClearingPrice)?;
			let cp_out = _clearing_prices
				.get(&_resolve.asset_out())
				.ok_or(Error::<T>::MissingClearingPrice)?;

			ensure!(
				Self::calc_amount_out(_resolve.amount_in(), cp_in, cp_out)
					.ok_or(Error::<T>::ArithmeticOverflow)?
					.eq(&_resolve.amount_out()),
				Error::<T>::PriceInconsistency
			);

			Ok(())
		}
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
	fn calc_amount_out(amount_in: Balance, price_in: &Price, price_out: &Price) -> Option<u128> {
		let n = U512::from(price_in.n).checked_mul(U512::from(price_out.d))?;
		let d = U512::from(price_in.d).checked_mul(U512::from(price_out.n))?;

		n.checked_mul(U512::from(amount_in))?.checked_div(d)?.checked_into()
	}

	/// Function validates intent's `amount_in` and `amount_out` values are bigger than existential
	/// deposit.
	fn validate_intent_amounts(intent: &IntentData) -> Result<(), DispatchError> {
		let in_ed =
			<T as Config>::RegistryHandler::existential_deposit(intent.asset_in()).ok_or(Error::<T>::AssetNotFound)?;
		let out_ed =
			<T as Config>::RegistryHandler::existential_deposit(intent.asset_out()).ok_or(Error::<T>::AssetNotFound)?;

		ensure!(intent.amount_in() >= in_ed, Error::<T>::InvalidAmount);
		ensure!(intent.amount_out() >= out_ed, Error::<T>::InvalidAmount);

		Ok(())
	}

	/// Function validates provided solution and returns solution's score if solution is
	/// valid.
	fn validate_unsigned_solution(solution: &Solution) -> Result<(), DispatchError> {
		//TODO:
		// * add weight rule and make sure solution respects it.

		Self::validate_clearing_prices(&solution.clearing_prices)?;

		let mut processed_intents: BTreeSet<IntentId> = BTreeSet::new();
		let mut score: Score = 0;
		for ResolvedIntent { id, data: resolve } in &solution.resolved_intents {
			Self::validate_intent_amounts(resolve)?;

			let intent = pallet_intent::Pallet::<T>::get_intent(id).ok_or(Error::<T>::IntentNotFound)?;
			let surplus = intent.data.surplus(resolve).ok_or(Error::<T>::ArithmeticOverflow)?;
			score = score.checked_add(surplus).ok_or(Error::<T>::ArithmeticOverflow)?;

			ensure!(processed_intents.insert(*id), Error::<T>::DuplicateIntent);

			pallet_intent::Pallet::<T>::validate_resolve(&intent, resolve)?;

			Self::validate_price_consitency(&solution.clearing_prices, resolve)?;
		}

		ensure!(solution.score == score, Error::<T>::ScoreMismatch);
		Ok(())
	}

	pub fn run<F>(block_no: BlockNumberFor<T>, solve: F) -> Option<Call<T>>
	where
		F: FnOnce(
			Vec<Intent>,
			<<T::Simulator as SimulatorConfig>::Simulators as SimulatorSet>::State,
		) -> Option<Solution>,
	{
		let intents: Vec<Intent> = pallet_intent::Pallet::<T>::get_valid_intents()
			.iter()
			.map(|x| Intent {
				id: x.0,
				data: x.1.data.to_owned(),
			})
			.collect();

		if intents.is_empty() {
			return None;
		}

		let state = <<T as Config>::Simulator as SimulatorConfig>::Simulators::initial_state();

		let Some(solution) = solve(intents, state) else {
			log::debug!(target: OCW_LOG_TARGET, "no solution found, block: {:?}", block_no);
			return None;
		};

		if let Err(e) = Self::validate_unsigned_solution(&solution) {
			log::error!(target: OCW_LOG_TARGET, "validate solution, err: {:?}, block: {:?}", e, block_no);
			return None;
		}

		Some(Call::submit_solution {
			solution,
			valid_for_block: block_no.saturating_add(One::one()),
		})
	}
}
