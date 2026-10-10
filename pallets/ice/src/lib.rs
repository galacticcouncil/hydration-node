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
#![allow(clippy::useless_conversion)]

#[cfg(test)]
mod tests;

pub mod traits;
mod weights;

use frame_support::dispatch::DispatchResult;
use frame_support::dispatch::PostDispatchInfo;
use frame_support::pallet_prelude::*;
use frame_support::storage::with_transaction;
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
use ice_support::BlockNumber;
use ice_support::Intent;
use ice_support::IntentData;
use ice_support::IntentId;
use ice_support::Partial;
use ice_support::PoolTrade;
use ice_support::Price;
use ice_support::ResolvedIntent;
use ice_support::Score;
use ice_support::Solution;
use ice_support::SolverMode;
use ice_support::SwapData;
use ice_support::SwapType;
use orml_traits::MultiCurrency;
use pallet_route_executor::AmmTradeWeights;
use sp_core::U256;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::traits::CheckedConversion;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_runtime::Permill;
use sp_runtime::TransactionOutcome;
use sp_std::borrow::ToOwned;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec::Vec;

pub use pallet::*;
pub use weights::WeightInfo;

pub const UNSIGNED_TXS_PRIORITY: u64 = u64::MAX;
/// Extra gas provided to EVM calls during solution execution.
const EXTRA_GAS: u64 = 1_000_000;
const LOG_TARGET: &str = "ice";
const OCW_LOG_TARGET: &str = "ice::offchain_worker";
const LOG_PREFIX: &str = "ICE#pallet_ice";
pub(crate) const OCW_TAG_PREFIX: &str = "ice-solution";
pub(crate) const OCW_PROVIDES: &[u8; 15] = b"submit_solution";

/// Parts returned by `solver_input`: valid intents, the SCALE-encoded simulator
/// snapshot, the ED map for every asset the solver may query, the per-intent
/// admission floors (DCA only), the matched fee, and the active solver mode.
pub type SolverInputParts = (
	Vec<Intent>,
	Vec<u8>,
	Vec<(AssetId, Balance)>,
	Vec<(IntentId, Balance)>,
	Permill,
	SolverMode,
);

/// How `submit_solution` validates and executes a solution. Every matching
/// solver shares `Strict`; only the emergency pass-through differs.
enum ExecutionPolicy {
	Strict,
	Passthrough,
}

trait SolverModePolicy {
	fn execution_policy(&self) -> Option<ExecutionPolicy>;
}

impl SolverModePolicy for SolverMode {
	fn execution_policy(&self) -> Option<ExecutionPolicy> {
		match self {
			SolverMode::V4 => Some(ExecutionPolicy::Strict),
			SolverMode::Passthrough => Some(ExecutionPolicy::Passthrough),
			SolverMode::Disabled => None,
		}
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use ice_support::SwapType;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_intent::Config + pallet_route_executor::Config<AssetId = AssetId>
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

		/// Default matched-volume protocol fee.
		///
		/// Charged on the portion of every resolved intent's `amount_out` that
		/// was matched directly against other intents (intent-to-intent
		/// matching, no AMM hop). Volume routed through AMMs pays only pool
		/// fees and is untouched. The user always receives `resolve.amount_out`
		/// as the net amount — the solver bakes the fee into matched pricing so
		/// the slippage minimum is honored.
		///
		/// Can be overridden via governance using `set_protocol_fee`.
		#[pallet::constant]
		type MatchedFee: Get<Permill>;

		/// Account that receives the collected matched-volume protocol fees.
		///
		/// At the end of each `submit_solution`, the matched-volume fee per
		/// touched asset is swept from the working holding pot to this account.
		type FeeReceiver: Get<Self::AccountId>;

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
			/// Block the solution was built against — subtract from the block
			/// carrying this event to get submit-to-execution drift.
			built_at: BlockNumber,
		},
		/// Protocol fee has been updated.
		ProtocolFeeSet { fee: Permill },
		/// Active solver mode has been updated.
		SolverModeSet { mode: SolverMode },
	}

	/// Matched-volume protocol fee.
	///
	/// Applied only to the share of each intent's volume that was matched
	/// intent-to-intent (without an AMM hop). Defaults to `T::MatchedFee`.
	/// Can be overridden via `set_protocol_fee`.
	#[pallet::storage]
	#[pallet::getter(fn protocol_fee)]
	pub type ProtocolFee<T: Config> = StorageValue<_, Permill, ValueQuery, T::MatchedFee>;

	/// Active solver mode. Absent storage means `SolverMode::V4` — the status quo.
	#[pallet::storage]
	#[pallet::getter(fn solver_mode)]
	pub type CurrentSolverMode<T: Config> = StorageValue<_, SolverMode, ValueQuery>;

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
		/// Holding-pot residual for some asset does not cover the expected
		/// matched-volume protocol fee — the solver under-priced matched
		/// volume, or claimed more matched volume than the trades support.
		FeeMismatch,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Execute `solution` submitted by OCW.
		///
		/// Under `SolverMode::V4` the solution is executed as a whole: claimed amounts are
		/// binding and any failure aborts the batch. Under `SolverMode::Passthrough` the
		/// claimed amounts and `score` are advisory — each intent is executed on its own,
		/// bound by the limit re-derived from its stored state, and a failing intent is
		/// skipped instead of failing the batch.
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
		pub fn submit_solution(origin: OriginFor<T>, solution: Solution) -> DispatchResultWithPostInfo {
			ensure_none(origin)?;

			match CurrentSolverMode::<T>::get().execution_policy() {
				None => return Err(Error::<T>::InvalidSolution.into()),
				Some(ExecutionPolicy::Passthrough) => return Self::execute_passthrough(&solution),
				Some(ExecutionPolicy::Strict) => {}
			}

			// Provide extra gas for EVM token transfers that may need it.
			T::ExtraGasSupport::set_extra_gas(EXTRA_GAS);

			log::debug!(target: LOG_TARGET, "{:?}: submit_solution() [EXECUTION PHASE], solution with {:?} resolved intents, {:?} trades, score: {:?}",
				LOG_PREFIX, solution.resolved_intents.len(), solution.trades.len(), solution.score);

			// V1 solver may produce solutions with no trades (perfect CoW matching)
			ensure!(!solution.resolved_intents.is_empty(), Error::<T>::InvalidSolution);

			let mut processed_intents: BTreeSet<IntentId> = BTreeSet::new();
			let holding_pot = Self::get_pallet_account();
			let holding_origin: OriginFor<T> = Origin::<T>::Signed(holding_pot.clone()).into();
			let fee_rate = Self::protocol_fee();

			// Per-asset flow accounting. After all transfers, the holding pot
			// residual per asset (intent_in + pool_out − intent_out − pool_in)
			// must cover `matched_X * fee_rate` where `matched_X = intent_in −
			// pool_in`. The fee is then swept to `T::FeeReceiver`. Anything
			// beyond the expected fee (pool positive slippage) accumulates in
			// the holding pot as today.
			let mut flows: BTreeMap<AssetId, AssetFlow> = BTreeMap::new();

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

				// Per-asset accumulation: intent input is X → holding pot.
				let entry_in = flows.entry(intent.asset_in()).or_default();
				entry_in.intent_in = entry_in.intent_in.saturating_add(intent.amount_in());
				let entry_out = flows.entry(intent.asset_out()).or_default();
				entry_out.intent_out = entry_out.intent_out.saturating_add(intent.amount_out());
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

				// Measure pool output via balance delta — the router's actual
				// payback may exceed the claimed minimum (positive slippage)
				// and we want to reflect the true amount in the residual check.
				let before_out = <T as Config>::Currency::free_balance(asset_out, &holding_pot);

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

				let after_out = <T as Config>::Currency::free_balance(asset_out, &holding_pot);
				let actual_pool_out = after_out.saturating_sub(before_out);

				let entry_in = flows.entry(asset_in).or_default();
				entry_in.pool_in = entry_in.pool_in.saturating_add(t.amount_in);
				let entry_out = flows.entry(asset_out).or_default();
				entry_out.pool_out = entry_out.pool_out.saturating_add(actual_pool_out);
			}

			let mut exec_score: Score = 0;
			let mut exec_prices: BTreeMap<(AssetId, AssetId), Price> = BTreeMap::new();
			for resolved_intent in &solution.resolved_intents {
				let ResolvedIntent { id, data: resolve } = resolved_intent;
				log::debug!(target: LOG_TARGET, "{LOG_PREFIX:?}: sumbit_solution(), resolving intent, id: {id:?}");

				ensure!(processed_intents.insert(*id), Error::<T>::DuplicateIntent);

				let owner = pallet_intent::Pallet::<T>::intent_owner(id).ok_or(Error::<T>::IntentOwnerNotFound)?;

				// `resolve.amount_out` is the NET amount delivered to the user
				// — the solver has already baked the matched-volume fee into
				// matched pricing. Transfer the full amount; the residual that
				// remains in the holding pot is the protocol fee, swept below.
				log::debug!(target: LOG_TARGET, "{:?}: sumbit_solution(), transferring, id: {:?}, to: {:?}, amount: {:?}", LOG_PREFIX, id, owner, resolve.amount_out());

				<T as Config>::Currency::transfer(
					resolve.asset_out(),
					&holding_pot,
					&owner,
					resolve.amount_out(),
					AllowDeath,
				)?;

				Self::validate_price_consistency(&mut exec_prices, resolve)?;

				let intent = pallet_intent::Pallet::<T>::get_intent(id).ok_or(Error::<T>::IntentNotFound)?;
				let surplus = pallet_intent::Pallet::<T>::compute_surplus(&intent, resolve)
					.ok_or(Error::<T>::ArithmeticOverflow)?;
				log::debug!(target: LOG_TARGET, "{LOG_PREFIX:?}: sumbit_solution(), id: {id:?}, surplus: {surplus:?}");
				exec_score = exec_score.checked_add(surplus).ok_or(Error::<T>::ArithmeticOverflow)?;

				pallet_intent::Pallet::<T>::intent_resolved(&owner, resolved_intent)?;
			}

			// Conservation: the holding pot must hold at least `matched_X * fee_rate`
			// per asset after all transfers. Sweep that fee to the dedicated
			// receiver account; any excess (pool positive slippage) stays in the pot.
			Self::settle_matched_fees(&flows, &holding_pot, fee_rate)?;

			log::debug!(target: LOG_TARGET, "{:?}: sumbit_solution(), solution execution finished, exec_score: {:?}, score: {:?}", LOG_PREFIX, exec_score, solution.score);
			ensure!(solution.score == exec_score, Error::<T>::ScoreMismatch);

			T::ExtraGasSupport::clear_extra_gas();

			Self::deposit_event(Event::SolutionExecuted {
				intents_executed: solution.resolved_intents.len() as u64,
				trades_executed: solution.trades.len() as u64,
				score: solution.score,
				built_at: solution.built_at,
			});

			Ok(().into())
		}

		/// Set the matched-volume protocol fee.
		///
		/// If `fee` matches the default constant (`T::MatchedFee`), the storage
		/// override is removed — there is no need to store the default value.
		///
		/// Can only be called by `AuthorityOrigin` (e.g. TechnicalCommittee or Root).
		///
		/// Emits `ProtocolFeeSet`.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::set_protocol_fee())]
		pub fn set_protocol_fee(origin: OriginFor<T>, fee: Permill) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			if fee == T::MatchedFee::get() {
				ProtocolFee::<T>::kill();
			} else {
				ProtocolFee::<T>::put(fee);
			}

			Self::deposit_event(Event::ProtocolFeeSet { fee });

			Ok(())
		}

		/// Select the active solver mode.
		///
		/// If `mode` is the default variant, the storage override is removed — there is
		/// no need to store the default value.
		///
		/// Can only be called by `AuthorityOrigin` (e.g. TechnicalCommittee or Root).
		///
		/// Parameters:
		/// - `mode`: solver mode to activate
		///
		/// Emits `SolverModeSet` event when successful.
		///
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::set_solver_mode())]
		pub fn set_solver_mode(origin: OriginFor<T>, mode: SolverMode) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			if mode == SolverMode::default() {
				CurrentSolverMode::<T>::kill();
			} else {
				CurrentSolverMode::<T>::put(mode);
			}

			Self::deposit_event(Event::SolverModeSet { mode });

			Ok(())
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

/// Per-asset flow totals used to verify the matched-volume fee invariant
/// after a solution has been executed.
///
/// The invariant for each touched asset `X` is:
///
/// ```text
/// holding_pot_residual_X = intent_in + pool_out − intent_out − pool_in
///                       ≥ (intent_in − pool_in) * fee_rate
///                       = matched_X * fee_rate
/// ```
///
/// `matched_X` is the volume of `X` that flowed intent-to-intent without
/// touching an AMM. The protocol fee on that volume is what stays in the
/// holding pot after every other transfer settles; it is then swept to
/// `T::FeeReceiver`. Pool positive slippage (`pool_out` exceeding the
/// solver's claimed minimum) lands on top and stays in the working pot.
#[derive(Default, Clone, Copy)]
struct AssetFlow {
	/// Sum of `resolve.amount_in` over resolved intents with `asset_in == X`.
	intent_in: Balance,
	/// Sum of `resolve.amount_out` over resolved intents with `asset_out == X`.
	intent_out: Balance,
	/// Sum of `trade.amount_in` over pool trades with `asset_in == X`.
	pool_in: Balance,
	/// Sum of actual amounts received by the holding pot from pool trades
	/// with `asset_out == X` (measured via balance delta — captures positive
	/// pool slippage). The claimed `trade.amount_out` is only a minimum.
	pool_out: Balance,
}

impl<T: Config> Pallet<T> {
	/// Function provides `holding_pot` account id.
	pub fn get_pallet_account() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	/// Emergency drain: execute each `(resolved_intent, trade)` pair on its own.
	///
	/// No matching, so no price consistency, no score match and no matched-volume
	/// fee — the only binding guarantee is each intent's own limit, re-derived from
	/// its stored state. A failing intent rolls back alone and the loop continues.
	fn execute_passthrough(solution: &Solution) -> DispatchResultWithPostInfo {
		log::debug!(target: LOG_TARGET, "{:?}: submit_solution() [PASSTHROUGH], solution with {:?} resolved intents, {:?} trades",
			LOG_PREFIX, solution.resolved_intents.len(), solution.trades.len());

		// Routes through EVM-bridged assets need the extra gas allowance.
		T::ExtraGasSupport::set_extra_gas(EXTRA_GAS);

		let mut executed: u64 = 0;
		let mut score: Score = 0;
		let mut actual_weight = Weight::zero();

		for (resolved_intent, trade) in solution.resolved_intents.iter().zip(solution.trades.iter()) {
			let id = resolved_intent.id;

			let outcome = with_transaction(|| match Self::passthrough_resolve_intent(id, trade) {
				Ok(surplus) => TransactionOutcome::Commit(Ok(surplus)),
				Err(e) => TransactionOutcome::Rollback(Err(e)),
			});

			match outcome {
				Ok(surplus) => {
					executed = executed.saturating_add(1);
					score = score.saturating_add(surplus);
					actual_weight = actual_weight
						.saturating_add(<T as Config>::WeightInfo::submit_solution())
						.saturating_add(<T as pallet_route_executor::Config>::WeightInfo::sell_weight(
							trade.route.as_slice(),
						));
				}
				Err(e) => {
					log::debug!(target: LOG_TARGET, "{LOG_PREFIX:?}: submit_solution() [PASSTHROUGH], intent {id:?} skipped, err: {e:?}");
				}
			}
		}

		T::ExtraGasSupport::clear_extra_gas();

		Self::deposit_event(Event::SolutionExecuted {
			intents_executed: executed,
			trades_executed: executed,
			score,
			built_at: solution.built_at,
		});

		Ok(PostDispatchInfo {
			actual_weight: Some(actual_weight),
			pays_fee: Pays::Yes,
		})
	}

	/// Settle one intent through the router and hand it to `intent_resolved`.
	/// Returns the intent's surplus over its own limit (observability only).
	fn passthrough_resolve_intent(id: IntentId, trade: &PoolTrade) -> Result<Balance, DispatchError> {
		let intent = pallet_intent::Pallet::<T>::get_intent(id).ok_or(Error::<T>::IntentNotFound)?;
		let owner = pallet_intent::Pallet::<T>::intent_owner(id).ok_or(Error::<T>::IntentOwnerNotFound)?;

		let asset_in = intent.data.asset_in();
		let asset_out = intent.data.asset_out();

		let (amount_in, min_limit, partial) = match intent.data {
			IntentData::Swap(ref swap) => {
				let amount_in = swap.remaining();
				let limit = if swap.partial.is_partial() {
					// Reuse `pro_rata` so the limit matches `validate_resolve`'s to the last unit.
					let probe = IntentData::Swap(SwapData {
						amount_in,
						..swap.clone()
					});
					intent.data.pro_rata(&probe).ok_or(Error::<T>::ArithmeticOverflow)?
				} else {
					swap.amount_out
				};
				(amount_in, limit, swap.partial)
			}
			IntentData::Dca(ref dca) => (
				dca.amount_in,
				pallet_intent::Pallet::<T>::compute_dca_effective_limit(dca),
				Partial::No,
			),
		};

		let mut resolve = SwapData {
			asset_in,
			asset_out,
			amount_in,
			amount_out: min_limit,
			partial,
		};
		Self::validate_intent_amounts(&IntentData::Swap(resolve.clone()))?;

		pallet_intent::Pallet::<T>::unlock_funds(&owner, asset_in, amount_in)?;

		let before_out = <T as Config>::Currency::free_balance(asset_out, &owner);

		log::debug!(target: LOG_TARGET, "{LOG_PREFIX:?}: submit_solution() [PASSTHROUGH], selling for intent {id:?}, owner: {owner:?}, asset_in: {asset_in:?}, amount_in: {amount_in:?}, asset_out: {asset_out:?}, min_amount_out: {min_limit:?}");

		pallet_route_executor::Pallet::<T>::sell(
			Origin::<T>::Signed(owner.clone()).into(),
			asset_in,
			asset_out,
			amount_in.into(),
			min_limit.into(),
			trade.route.clone(),
		)?;

		// The router pays out to the owner directly — no holding pot involved.
		resolve.amount_out = <T as Config>::Currency::free_balance(asset_out, &owner).saturating_sub(before_out);

		let resolve = IntentData::Swap(resolve);
		let surplus = pallet_intent::Pallet::<T>::compute_surplus(&intent, &resolve).unwrap_or_default();

		pallet_intent::Pallet::<T>::intent_resolved(&owner, &ResolvedIntent { id, data: resolve })?;

		Ok(surplus)
	}

	/// Verify the matched-volume fee invariant and sweep collected fees to the
	/// dedicated `T::FeeReceiver` account.
	fn settle_matched_fees(
		flows: &BTreeMap<AssetId, AssetFlow>,
		holding_pot: &T::AccountId,
		fee_rate: Permill,
	) -> DispatchResult {
		let fee_receiver = T::FeeReceiver::get();
		for (asset, f) in flows.iter() {
			let matched = f.intent_in.saturating_sub(f.pool_in);
			let expected_fee = fee_rate.mul_floor(matched);

			let inflows = f
				.intent_in
				.checked_add(f.pool_out)
				.ok_or(Error::<T>::ArithmeticOverflow)?;
			let outflows = f
				.intent_out
				.checked_add(f.pool_in)
				.ok_or(Error::<T>::ArithmeticOverflow)?;
			let residual = inflows.checked_sub(outflows).ok_or(Error::<T>::FeeMismatch)?;
			ensure!(residual >= expected_fee, Error::<T>::FeeMismatch);

			// Only sweep when the fee clears the asset's existential deposit.
			// A sub-ED transfer to the (non-dust-protected) receiver would be
			// rejected by the currency layer and brick the whole solution, so
			// sub-ED fees are left in the holding pot (governance drains it).
			let ed = <T as Config>::RegistryHandler::existential_deposit(*asset).unwrap_or(Balance::MAX);
			if expected_fee >= ed && holding_pot != &fee_receiver {
				<T as Config>::Currency::transfer(*asset, holding_pot, &fee_receiver, expected_fee, AllowDeath)?;
				log::debug!(target: LOG_TARGET, "{LOG_PREFIX:?}: settle_matched_fees(), swept asset: {asset:?}, fee: {expected_fee:?} -> {fee_receiver:?}");
			} else if expected_fee > 0 {
				log::debug!(target: LOG_TARGET, "{LOG_PREFIX:?}: settle_matched_fees(), fee {expected_fee:?} for asset {asset:?} below ED {ed:?} — retained in pot");
			}
		}
		Ok(())
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

	/// Whether the active solver mode can settle intents at all.
	///
	/// `Disabled` has no execution policy, so nothing can settle; every other mode
	/// can. Exposed so the runtime can refuse new intents while settlement is off,
	/// keeping the mode-to-capability mapping in one place.
	pub fn settlement_enabled() -> bool {
		CurrentSolverMode::<T>::get().execution_policy().is_some()
	}

	/// Function validates provided solution against the active solver mode.
	fn validate_unsigned_solution(solution: &Solution) -> Result<(), DispatchError> {
		match CurrentSolverMode::<T>::get().execution_policy() {
			None => Err(Error::<T>::InvalidSolution.into()),
			Some(ExecutionPolicy::Strict) => Self::validate_strict_solution(solution),
			Some(ExecutionPolicy::Passthrough) => Self::validate_passthrough_solution(solution),
		}
	}

	/// Shape-only validation for pass-through solutions: 1:1 intents/trades, no
	/// duplicates, every intent known, every route wired to its intent's pair.
	///
	/// Claimed amounts and `score` are deliberately unchecked — execution derives
	/// the binding amounts from stored intent state and ignores them.
	fn validate_passthrough_solution(solution: &Solution) -> Result<(), DispatchError> {
		ensure!(!solution.resolved_intents.is_empty(), Error::<T>::InvalidSolution);
		ensure!(
			solution.resolved_intents.len() == solution.trades.len(),
			Error::<T>::InvalidSolution
		);

		let mut processed_intents: BTreeSet<IntentId> = BTreeSet::new();

		for (resolved_intent, trade) in solution.resolved_intents.iter().zip(solution.trades.iter()) {
			let id = resolved_intent.id;

			if !processed_intents.insert(id) {
				log::error!(target: OCW_LOG_TARGET, "{LOG_PREFIX:?}: validate_passthrough_solution(), intent {id:?} is duplicate");
				return Err(Error::<T>::DuplicateIntent.into());
			}

			let intent = pallet_intent::Pallet::<T>::get_intent(id).ok_or_else(|| {
				log::error!(target: OCW_LOG_TARGET, "{LOG_PREFIX:?}: validate_passthrough_solution(), intent {id:?} not found in storage");
				Error::<T>::IntentNotFound
			})?;

			ensure!(trade.direction == SwapType::ExactIn, Error::<T>::InvalidSolution);

			let route_in = trade.route.first().ok_or(Error::<T>::InvalidRoute)?.asset_in;
			let route_out = trade.route.last().ok_or(Error::<T>::InvalidRoute)?.asset_out;
			ensure!(route_in == intent.data.asset_in(), Error::<T>::InvalidRoute);
			ensure!(route_out == intent.data.asset_out(), Error::<T>::InvalidRoute);
		}

		Ok(())
	}

	/// Function validates provided solution and returns solution's score if solution is
	/// valid.
	fn validate_strict_solution(solution: &Solution) -> Result<(), DispatchError> {
		//TODO:
		// * add weight rule and make sure solution respects it.

		let mut processed_intents: BTreeSet<IntentId> = BTreeSet::new();
		let mut score: Score = 0;
		let mut exec_prices: BTreeMap<(AssetId, AssetId), Price> = BTreeMap::new();

		// NOTE: the matched-volume fee conservation invariant is intentionally
		// NOT checked here. It depends on the *actual* pool output, which is
		// only known at execution. The solver's claimed `trade.amount_out` is a
		// lower bound (pools can return more — positive slippage), so verifying
		// the residual offchain against the claimed minimum would produce false
		// rejections of solutions that balance fine at execution. The invariant
		// is enforced authoritatively in `submit_solution` via measured pool
		// output (see `settle_matched_fees`).
		for ResolvedIntent { id, data: resolve } in &solution.resolved_intents {
			log::debug!(target: OCW_LOG_TARGET, "{LOG_PREFIX:?}: validate_unsigned_solution(), resolved intent, id: {id:?}");

			if let Err(e) = Self::validate_intent_amounts(resolve) {
				log::error!(target: OCW_LOG_TARGET, "{LOG_PREFIX:?}: validate_unsigned_solution(), intent {id:?} failed amount validation: {e:?}");
				return Err(e);
			}

			let intent = pallet_intent::Pallet::<T>::get_intent(id).ok_or_else(|| {
				log::error!(target: OCW_LOG_TARGET, "{LOG_PREFIX:?}: validate_unsigned_solution(), intent {id:?} not found in storage");
				Error::<T>::IntentNotFound
			})?;

			let surplus =
				pallet_intent::Pallet::<T>::compute_surplus(&intent, resolve).ok_or(Error::<T>::ArithmeticOverflow)?;
			log::debug!(target: OCW_LOG_TARGET, "{LOG_PREFIX:?}: validate_unsigned_solution(), id: {id:?}, surplus: {surplus:?}");
			score = score.checked_add(surplus).ok_or(Error::<T>::ArithmeticOverflow)?;

			if !processed_intents.insert(*id) {
				log::error!(target: OCW_LOG_TARGET, "{LOG_PREFIX:?}: validate_unsigned_solution(), intent {id:?} is duplicate");
				return Err(Error::<T>::DuplicateIntent.into());
			}

			if let Err(e) = pallet_intent::Pallet::<T>::validate_resolve(&intent, resolve) {
				log::error!(target: OCW_LOG_TARGET, "{LOG_PREFIX:?}: validate_unsigned_solution(), intent {id:?} failed resolve validation: {e:?}");
				return Err(e);
			}

			if let Err(e) = Self::validate_price_consistency(&mut exec_prices, resolve) {
				log::error!(target: OCW_LOG_TARGET, "{LOG_PREFIX:?}: validate_unsigned_solution(), intent {id:?} failed price consistency: {e:?}");
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

	/// Build the side-effect-free inputs for the node-side solver: the valid
	/// intents, the SCALE-encoded simulator snapshot, the ED map for every asset
	/// the solver may query, the per-intent admission floors, the matched-volume
	/// fee and the active solver mode. Returns `None` on an empty intent set (the
	/// cheap empty-block path — no snapshot/EVM work).
	pub fn solver_input() -> Option<SolverInputParts> {
		let (valid, min_amounts_out) = pallet_intent::Pallet::<T>::solver_intents();
		let intents: Vec<Intent> = valid
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

		// ED universe = snapshot pool assets ∪ intent assets — exactly the set the
		// solver can query. Source EDs via the simulator config (the same source
		// the wasm solve uses) so the node's native solve is byte-identical.
		let mut assets = BTreeSet::<AssetId>::new();
		for edge in <<T as Config>::Simulator as SimulatorConfig>::Simulators::pool_edges(&state) {
			assets.extend(edge.assets);
		}
		for intent in &intents {
			assets.insert(intent.data.asset_in());
			assets.insert(intent.data.asset_out());
		}
		let existential_deposits = assets
			.into_iter()
			.map(|asset| {
				(
					asset,
					<<T as Config>::Simulator as SimulatorConfig>::existential_deposit(asset),
				)
			})
			.collect();

		Some((
			intents,
			state.encode(),
			existential_deposits,
			min_amounts_out,
			Self::protocol_fee(),
			CurrentSolverMode::<T>::get(),
		))
	}

	/// Solve-and-validate, mirroring what the node worker does with
	/// `solver_input`. `solve` receives the intents, the per-intent admission
	/// floors and the simulator snapshot.
	pub fn run<F>(block_no: BlockNumberFor<T>, solve: F) -> Option<Call<T>>
	where
		F: FnOnce(
			Vec<Intent>,
			Vec<(IntentId, Balance)>,
			<<T::Simulator as SimulatorConfig>::Simulators as SimulatorSet>::State,
		) -> Option<Solution>,
	{
		let (valid, min_amounts_out) = pallet_intent::Pallet::<T>::solver_intents();
		let intents: Vec<Intent> = valid
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

		let Some(mut solution) = solve(intents, min_amounts_out, state) else {
			log::debug!(target: OCW_LOG_TARGET, "{LOG_PREFIX:?}: solver returned no solution, block: {block_no:?}");
			return None;
		};
		solution.built_at = block_no.unique_saturated_into();

		if solution.resolved_intents.is_empty() {
			log::debug!(target: OCW_LOG_TARGET, "{LOG_PREFIX:?}: solver returned empty solution (no resolvable intents), block: {block_no:?}");
			return None;
		}

		if let Err(e) = Self::validate_unsigned_solution(&solution) {
			log::error!(target: OCW_LOG_TARGET, "{LOG_PREFIX:?}: validate solution, err: {e:?}, block: {block_no:?}");
			return None;
		}

		Some(Call::submit_solution { solution })
	}
}
