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
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use hydradx_traits::evm::ExtraGasSupport;
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
use orml_traits::MultiCurrency;
use pallet_route_executor::AmmTradeWeights;
use sp_core::U256;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::traits::CheckedConversion;
use sp_runtime::Permill;
use sp_std::borrow::ToOwned;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec::Vec;

pub use pallet::*;
pub use weights::WeightInfo;

pub const UNSIGNED_TXS_PRIORITY: u64 = u64::max_value();
/// Extra gas provided to EVM calls during solution execution.
const EXTRA_GAS: u64 = 1_000_000;
const LOG_TARGET: &str = "ice";
const OCW_LOG_TARGET: &str = "ice::offchain_worker";
const LOG_PREFIX: &str = "ICE#pallet_ice";
pub(crate) const OCW_TAG_PREFIX: &str = "ice-solution";
pub(crate) const OCW_PROVIDES: &[u8; 15] = b"submit_solution";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_system::offchain::SubmitTransaction;
	use hydradx_traits::CreateBare;
	use ice_solver::v2::Solver;
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
		/// Multi currency mechanism
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = AssetId, Balance = Balance>;

		/// Pallet id - used to create a holding account
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Asset registry handler
		type RegistryHandler: Inspect<AssetId = AssetId>;

		/// Simulator configuration - provides simulators and route provider for the solver
		type Simulator: SimulatorConfig;

		/// Default protocol fee taken from each resolved intent's output amount.
		/// Fee stays in the ICE holding account.
		/// Can be overridden via governance using `set_protocol_fee`.
		#[pallet::constant]
		type Fee: Get<Permill>;

		/// Origin that can set the protocol fee (e.g. TechnicalCommittee or Root).
		type AuthorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Extra gas support for EVM token transfers.
		type ExtraGasSupport: ExtraGasSupport;

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
		/// Protocol fee has been updated.
		ProtocolFeeSet { fee: Permill },
	}

	/// Protocol fee taken from each resolved intent's output amount.
	/// Defaults to `T::Fee` constant. Can be overridden via `set_protocol_fee`.
	#[pallet::storage]
	#[pallet::getter(fn protocol_fee)]
	pub type ProtocolFee<T: Config> = StorageValue<_, Permill, ValueQuery, T::Fee>;

	#[pallet::error]
	pub enum Error<T> {
		/// Provided solution is not valid.
		InvalidSolution,
		/// Referenced intent doesn't exist.
		IntentNotFound,
		/// Referenced intent's owner doesn't exist.
		IntentOwnerNotFound,
		/// Resolution violates user's limit.
		LimitViolation,
		/// Trade price doesn't match execution price.
		PriceInconsistency,
		/// Intent was referenced multiple times.
		DuplicateIntent,
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
		///
		/// Emits:
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
		pub fn submit_solution(origin: OriginFor<T>, solution: Solution) -> DispatchResult {
			ensure_none(origin)?;

			// Provide extra gas for EVM token transfers that may need it.
			T::ExtraGasSupport::set_extra_gas(EXTRA_GAS);

			log::debug!(target: LOG_TARGET, "{:?}: submit_solution() [EXECUTION PHASE], solution with {:?} resolved intents, {:?} trades, score: {:?}",
				LOG_PREFIX, solution.resolved_intents.len(), solution.trades.len(), solution.score);

			// V1 solver may produce solutions with no trades (perfect CoW matching)
			ensure!(!solution.resolved_intents.is_empty(), Error::<T>::InvalidSolution);

			let mut processed_intents: BTreeSet<IntentId> = BTreeSet::new();
			let holding_pot = Self::get_pallet_account();
			let holding_origin: OriginFor<T> = Origin::<T>::Signed(holding_pot.clone()).into();

			// TODO: this is not most perform solution, verify it works and optimize

			for ResolvedIntent { id, data: intent } in &solution.resolved_intents {
				Self::validate_intent_amounts(intent)?;

				let owner = pallet_intent::Pallet::<T>::intent_owner(id).ok_or(Error::<T>::IntentOwnerNotFound)?;
				pallet_intent::Pallet::<T>::unlock_funds(&owner, intent.asset_in(), intent.amount_in())?;

				log::debug!(target: LOG_TARGET, "{:?}: sumbit_solution(), unlock and transfer amounts, owner: {:?}, asset: {:?}, amount: {:?}",
					LOG_PREFIX, owner, intent.asset_in(), intent.amount_in());

				<T as Config>::Currency::transfer(
					intent.asset_in(),
					&owner,
					&holding_pot,
					intent.amount_in(),
					AllowDeath,
				)?;
			}

			for t in &solution.trades {
				let asset_in = t.route.first().ok_or(Error::<T>::InvalidRoute)?.asset_in;
				let asset_out = t.route.last().ok_or(Error::<T>::InvalidRoute)?.asset_out;

				let ed_in = <T as Config>::RegistryHandler::existential_deposit(asset_in).unwrap_or(Balance::MAX);
				let ed_out = <T as Config>::RegistryHandler::existential_deposit(asset_out).unwrap_or(Balance::MAX);

				// Skip dust trades where the amount is below the existential deposit —
				// near-perfect intent matching can leave a negligible AMM remainder.
				if t.amount_in < ed_in || t.amount_out < ed_out {
					log::debug!(target: LOG_TARGET, "{:?}: submit_solution(), skipping dust trade: amount_in {:?} (ed {:?}), amount_out {:?} (ed {:?})",
						LOG_PREFIX, t.amount_in, ed_in, t.amount_out, ed_out);
					continue;
				}

				match t.direction {
					SwapType::ExactOut => {
						log::debug!(target: LOG_TARGET, "{:?}: sumbit_solution(), buying, asset_in: {:?}, asset_out: {:?}, amount_out: {:?}, max_amount_in: {:?}, route: {:?}",
							LOG_PREFIX, t.route.first(), t.route.last(), t.amount_out, t.amount_in, t.route);

						pallet_route_executor::Pallet::<T>::buy(
							holding_origin.clone(),
							asset_in,
							asset_out,
							t.amount_out.into(),
							t.amount_in.into(),
							t.route.clone(),
						)?;
					}
					SwapType::ExactIn => {
						log::debug!(target: LOG_TARGET, "{:?}: sumbit_solution(), selling, asset_in: {:?}, asset_out: {:?}, amount_in: {:?}, min_amount_out: {:?}, route: {:?}",
							LOG_PREFIX, t.route.first(), t.route.last(), t.amount_in, t.amount_out, t.route);

						pallet_route_executor::Pallet::<T>::sell(
							holding_origin.clone(),
							asset_in,
							asset_out,
							t.amount_in.into(),
							t.amount_out.into(),
							t.route.clone(),
						)?;
					}
				}
			}

			let mut exec_score: Score = 0;
			let mut exec_prices: BTreeMap<(AssetId, AssetId), Price> = BTreeMap::new();
			for resolved_intent in &solution.resolved_intents {
				let ResolvedIntent { id, data: resolve } = resolved_intent;
				log::debug!(target: LOG_TARGET, "{:?}: sumbit_solution(), resolving intent, id: {:?}", LOG_PREFIX, id);

				ensure!(processed_intents.insert(*id), Error::<T>::DuplicateIntent);

				let owner = pallet_intent::Pallet::<T>::intent_owner(id).ok_or(Error::<T>::IntentOwnerNotFound)?;

				let fee_amount = Self::protocol_fee().mul_floor(resolve.amount_out());
				let payout = resolve.amount_out().saturating_sub(fee_amount);

				log::debug!(target: LOG_TARGET, "{:?}: sumbit_solution(), transferring, id: {:?}, to: {:?}, amount: {:?}, fee: {:?}", LOG_PREFIX, id, owner, payout, fee_amount);

				<T as Config>::Currency::transfer(resolve.asset_out(), &holding_pot, &owner, payout, AllowDeath)?;

				Self::validate_price_consistency(&mut exec_prices, resolve)?;

				let intent = pallet_intent::Pallet::<T>::get_intent(id).ok_or(Error::<T>::IntentNotFound)?;
				let surplus = pallet_intent::Pallet::<T>::compute_surplus(&intent, resolve)
					.ok_or(Error::<T>::ArithmeticOverflow)?;
				log::debug!(target: LOG_TARGET, "{:?}: sumbit_solution(), id: {:?}, surplus: {:?}", LOG_PREFIX, id, surplus);
				exec_score = exec_score.checked_add(surplus).ok_or(Error::<T>::ArithmeticOverflow)?;

				pallet_intent::Pallet::<T>::intent_resolved(&owner, resolved_intent, fee_amount)?;
			}

			log::debug!(target: LOG_TARGET, "{:?}: sumbit_solution(), solution execution finished, exec_score: {:?}, score: {:?}", LOG_PREFIX, exec_score, solution.score);
			ensure!(solution.score == exec_score, Error::<T>::ScoreMismatch);

			T::ExtraGasSupport::clear_extra_gas();

			Self::deposit_event(Event::SolutionExecuted {
				intents_executed: solution.resolved_intents.len() as u64,
				trades_executed: solution.trades.len() as u64,
				score: solution.score,
			});

			Ok(())
		}

		/// Set the protocol fee for resolved intents.
		///
		/// If `fee` matches the default constant (`T::Fee`), the storage override
		/// is removed — there is no need to store the default value.
		///
		/// Can only be called by `AuthorityOrigin` (e.g. TechnicalCommittee or Root).
		///
		/// Emits `ProtocolFeeSet`.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::set_protocol_fee())]
		pub fn set_protocol_fee(origin: OriginFor<T>, fee: Permill) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			if fee == T::Fee::get() {
				ProtocolFee::<T>::kill();
			} else {
				ProtocolFee::<T>::put(fee);
			}

			Self::deposit_event(Event::ProtocolFeeSet { fee });

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(_n: BlockNumberFor<T>) {}

		fn offchain_worker(block_number: BlockNumberFor<T>) {
			let Some(call) = Self::run(block_number, |intents, state| {
				match Solver::<amm_simulator::HydrationSimulator<T::Simulator>>::solve(intents, state) {
					Ok(solution) => Some(solution),
					Err(e) => {
						log::error!(target: OCW_LOG_TARGET, "{:?}: solver failed, err: {:?}", LOG_PREFIX, e);
						None
					}
				}
			}) else {
				return;
			};

			let tx = <T as CreateBare<self::Call<T>>>::create_bare(call.into());
			if let Err(e) = SubmitTransaction::<T, Call<T>>::submit_transaction(tx) {
				log::error!(target: OCW_LOG_TARGET, "{:?}: submit_transaction failed (validate_unsigned rejected the solution), err: {:?}", LOG_PREFIX, e);
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

			if let Call::submit_solution { solution } = call {
				if let Err(e) = Self::validate_unsigned_solution(solution) {
					log::error!(target: OCW_LOG_TARGET, "{:?}: validate_unsigned rejected solution ({} intents, {} trades, score: {}), err: {:?}",
						LOG_PREFIX, solution.resolved_intents.len(), solution.trades.len(), solution.score, e);
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

	/// Function validates if intent was resolved based on execution price.
	/// Execution prices are computed on demand based on first trade trading `resolve`'s assets in same
	/// direction.
	/// `exeuction_prices` are [out/in] => [in] * [out/in] = [out]
	fn validate_price_consistency(
		execution_prices: &mut BTreeMap<(AssetId, AssetId), Price>,
		resolve: &IntentData,
	) -> Result<(), DispatchError> {
		{
			let asset_in = resolve.asset_in();
			let asset_out = resolve.asset_out();

			let exec_price = if let Some(ep) = execution_prices.get(&(asset_in, asset_out)) {
				ep
			} else {
				let new_price = Ratio {
					n: resolve.amount_out(),
					d: resolve.amount_in(),
				};

				execution_prices.insert((asset_in, asset_out), new_price);

				&new_price.clone()
			};

			let expected_out: u128 = U256::from(resolve.amount_in())
				.checked_mul(U256::from(exec_price.n))
				.ok_or(Error::<T>::ArithmeticOverflow)?
				.checked_div(U256::from(exec_price.d))
				.ok_or(Error::<T>::ArithmeticOverflow)?
				.checked_into()
				.ok_or(Error::<T>::ArithmeticOverflow)?;

			log::debug!(target: OCW_LOG_TARGET, "{:?}: validate_price_consistency(), price: {:?}, amount_in: {:?}, calculated_out: {:?}, intent_out: {:?}",
						LOG_PREFIX, exec_price, resolve.amount_in(), expected_out, resolve.amount_out());

			ensure!(
				expected_out.abs_diff(resolve.amount_out()) <= 1,
				Error::<T>::PriceInconsistency
			);

			Ok(())
		}
	}

	/// Function validates intent's `amount_in` and `amount_out` values are bigger than existential
	/// deposit.
	fn validate_intent_amounts(intent: &IntentData) -> Result<(), DispatchError> {
		let ed_in =
			<T as Config>::RegistryHandler::existential_deposit(intent.asset_in()).ok_or(Error::<T>::AssetNotFound)?;
		let ed_out =
			<T as Config>::RegistryHandler::existential_deposit(intent.asset_out()).ok_or(Error::<T>::AssetNotFound)?;

		log::debug!(target: LOG_TARGET, "{:?}: validate_intent_amounts(), ed_in: {:?}, amount_in: {:?}, ed_out: {:?}, amount_out: {:?}",
			LOG_PREFIX, ed_in, intent.amount_in(), ed_out, intent.amount_out());

		if intent.amount_in() < ed_in {
			log::error!(target: LOG_TARGET, "{:?}: validate_intent_amounts() FAILED: amount_in {:?} < existential_deposit {:?} for asset {:?}",
				LOG_PREFIX, intent.amount_in(), ed_in, intent.asset_in());
			return Err(Error::<T>::InvalidAmount.into());
		}
		if intent.amount_out() < ed_out {
			log::error!(target: LOG_TARGET, "{:?}: validate_intent_amounts() FAILED: amount_out {:?} < existential_deposit {:?} for asset {:?}",
				LOG_PREFIX, intent.amount_out(), ed_out, intent.asset_out());
			return Err(Error::<T>::InvalidAmount.into());
		}

		Ok(())
	}

	/// Function validates provided solution and returns solution's score if solution is
	/// valid.
	fn validate_unsigned_solution(solution: &Solution) -> Result<(), DispatchError> {
		//TODO:
		// * add weight rule and make sure solution respects it.

		let mut processed_intents: BTreeSet<IntentId> = BTreeSet::new();
		let mut score: Score = 0;
		let mut exec_prices: BTreeMap<(AssetId, AssetId), Price> = BTreeMap::new();
		for ResolvedIntent { id, data: resolve } in &solution.resolved_intents {
			log::debug!(target: OCW_LOG_TARGET, "{:?}: validate_unsigned_solution(), resolved intent, id: {:?}", LOG_PREFIX, id);

			if let Err(e) = Self::validate_intent_amounts(resolve) {
				log::error!(target: OCW_LOG_TARGET, "{:?}: validate_unsigned_solution(), intent {:?} failed amount validation: {:?}", LOG_PREFIX, id, e);
				return Err(e);
			}

			let intent = pallet_intent::Pallet::<T>::get_intent(id).ok_or_else(|| {
				log::error!(target: OCW_LOG_TARGET, "{:?}: validate_unsigned_solution(), intent {:?} not found in storage", LOG_PREFIX, id);
				Error::<T>::IntentNotFound
			})?;

			let surplus =
				pallet_intent::Pallet::<T>::compute_surplus(&intent, resolve).ok_or(Error::<T>::ArithmeticOverflow)?;
			log::debug!(target: OCW_LOG_TARGET, "{:?}: validate_unsigned_solution(), id: {:?}, surplus: {:?}", LOG_PREFIX, id, surplus);
			score = score.checked_add(surplus).ok_or(Error::<T>::ArithmeticOverflow)?;

			if !processed_intents.insert(*id) {
				log::error!(target: OCW_LOG_TARGET, "{:?}: validate_unsigned_solution(), intent {:?} is duplicate", LOG_PREFIX, id);
				return Err(Error::<T>::DuplicateIntent.into());
			}

			if let Err(e) = pallet_intent::Pallet::<T>::validate_resolve(&intent, resolve) {
				log::error!(target: OCW_LOG_TARGET, "{:?}: validate_unsigned_solution(), intent {:?} failed resolve validation: {:?}", LOG_PREFIX, id, e);
				return Err(e);
			}

			if let Err(e) = Self::validate_price_consistency(&mut exec_prices, resolve) {
				log::error!(target: OCW_LOG_TARGET, "{:?}: validate_unsigned_solution(), intent {:?} failed price consistency: {:?}", LOG_PREFIX, id, e);
				return Err(e);
			}
		}

		log::debug!(target: OCW_LOG_TARGET, "{:?}: validate_unsigned_solution(), exec_score: {:?}, score: {:?}", LOG_PREFIX, score, solution.score);
		if solution.score != score {
			log::error!(target: OCW_LOG_TARGET, "{:?}: validate_unsigned_solution(), score mismatch: solution claims {:?}, computed {:?}", LOG_PREFIX, solution.score, score);
			return Err(Error::<T>::ScoreMismatch.into());
		}
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

		log::debug!(target: OCW_LOG_TARGET, "{:?}: run(), block: {:?}, valid intents: {:?}", LOG_PREFIX, block_no, intents.len());

		if intents.is_empty() {
			return None;
		}

		let state = <<T as Config>::Simulator as SimulatorConfig>::Simulators::initial_state();

		let Some(solution) = solve(intents, state) else {
			log::debug!(target: OCW_LOG_TARGET, "{:?}: solver returned no solution, block: {:?}", LOG_PREFIX, block_no);
			return None;
		};

		if solution.resolved_intents.is_empty() {
			log::debug!(target: OCW_LOG_TARGET, "{:?}: solver returned empty solution (no resolvable intents), block: {:?}", LOG_PREFIX, block_no);
			return None;
		}

		if let Err(e) = Self::validate_unsigned_solution(&solution) {
			log::error!(target: OCW_LOG_TARGET, "{:?}: validate solution, err: {:?}, block: {:?}", LOG_PREFIX, e, block_no);
			return None;
		}

		Some(Call::submit_solution { solution })
	}
}
