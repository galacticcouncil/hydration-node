//!
#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

pub mod engine;
#[cfg(test)]
mod tests;
pub mod traits;
pub mod types;
pub mod validity;
mod weights;

use crate::traits::Routing;
use crate::traits::Solver;
use crate::types::{
	Balance, BoundedRoute, IncrementalIntentId, Intent, IntentId, Moment, NamedReserveIdentifier, ResolvedIntent,
	TradeInstruction,
};
use codec::{HasCompact, MaxEncodedLen};
use frame_support::pallet_prelude::StorageValue;
use frame_support::pallet_prelude::*;
use frame_support::traits::fungibles::{Inspect, Mutate};
use frame_support::traits::tokens::{Fortitude, Preservation};
use frame_support::traits::Time;
use frame_support::{dispatch::DispatchResult, traits::Get};
use frame_support::{Blake2_128Concat, Parameter};
use frame_system::pallet_prelude::*;
use hydradx_traits::router::RouterT;
use orml_traits::NamedMultiReservableCurrency;
pub use pallet::*;
use scale_info::TypeInfo;
use sp_core::offchain::Duration;
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::offchain::storage_lock::StorageLock;
use sp_runtime::traits::{AccountIdConversion, BlockNumberProvider};
use sp_runtime::traits::{MaybeSerializeDeserialize, Member, Zero};
use sp_runtime::Saturating;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::prelude::*;
pub use weights::WeightInfo;

pub const SOLVER_LOCK: &[u8] = b"hydradx/ice/lock/";
pub const LOCK_TIMEOUT_EXPIRATION: u64 = 5_000; // 5 seconds

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::engine::ICEEngine;
	use crate::traits::IceWeightBounds;
	use crate::types::{BoundedResolvedIntents, BoundedTrades, ResolvedIntent, TradeInstruction};
	use frame_support::traits::fungibles::Mutate;
	use frame_support::PalletId;
	use hydra_dx_math::ratio::Ratio;
	use hydradx_traits::price::PriceProvider;
	use sp_runtime::traits::BlockNumberProvider;
	use types::Balance;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset type.
		type AssetId: Member
			+ Parameter
			+ Default
			+ Copy
			+ Ord
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;

		/// Native asset Id
		#[pallet::constant]
		type NativeAssetId: Get<Self::AssetId>;

		/// Asset Id of hub asset
		#[pallet::constant]
		type HubAssetId: Get<Self::AssetId>;

		/// Provider for the current timestamp.
		type TimestampProvider: Time<Moment = Moment>;

		/// Maximum deadline for intent in milliseconds.
		#[pallet::constant]
		type MaxAllowedIntentDuration: Get<Moment>;

		/// Block number provider.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		/// TODO: this two currencies could be merged into one, however it would need to implement support in the runtime for this
		type Currency: Mutate<Self::AccountId, AssetId = Self::AssetId, Balance = types::Balance>;

		type ReservableCurrency: NamedMultiReservableCurrency<
			Self::AccountId,
			ReserveIdentifier = NamedReserveIdentifier,
			CurrencyId = Self::AssetId,
			Balance = Balance,
		>;

		type TradeExecutor: RouterT<
			Self::RuntimeOrigin,
			Self::AssetId,
			Balance,
			hydradx_traits::router::Trade<Self::AssetId>,
			hydradx_traits::router::AmountInAndOut<Balance>,
		>;

		/// The means of determining a solution's weight.
		type Weigher: IceWeightBounds<Self::RuntimeCall, Vec<hydradx_traits::router::Trade<Self::AssetId>>>;

		/// Price provider
		type PriceProvider: PriceProvider<Self::AssetId, Price = Ratio>;

		type RoutingSupport: Routing<Self::AssetId>;

		type Solver: traits::Solver<(IntentId, Intent<Self::AccountId, Self::AssetId>), Error = ()>;

		/// Pallet id.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		#[pallet::constant]
		type MaxCallData: Get<u32>;

		/// The bond required to propose a new solution.
		#[pallet::constant]
		type ProposalBond: Get<Balance>;

		/// The account which receives slashed bonds in case of invalid solution.
		#[pallet::constant]
		type SlashReceiver: Get<Self::AccountId>;

		#[pallet::constant]
		type NamedReserveId: Get<NamedReserveIdentifier>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New intent was submitted
		IntentSubmitted(IntentId, Intent<T::AccountId, T::AssetId>),

		/// Solution was executed
		SolutionExecuted { who: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// No more intent ids available
		IntendIdsExhausted,

		/// Data too long
		TooLong,

		/// Intent not found
		IntentNotFound,

		/// Price is missing in provided solution
		MissingPrice,

		/// Invalid block number
		InvalidBlockNumber,

		/// Invalid deadline
		InvalidDeadline,

		/// Insufficient reserved balance
		InsufficientReservedBalance,

		/// Invalid solution score
		InvalidScore,

		///
		IncorrectIntentAmountResolution,

		///
		InvalidTransferInstruction,

		///
		IntentLimitPriceViolation,

		/// Solution already executed in this block
		AlreadyExecuted,

		/// Invalid intent parameters
		InvalidIntent,
	}

	#[pallet::storage]
	#[pallet::getter(fn get_intent)]
	pub(super) type Intents<T: Config> = StorageMap<_, Blake2_128Concat, IntentId, Intent<T::AccountId, T::AssetId>>;

	#[pallet::storage]
	/// Intent id sequencer
	#[pallet::getter(fn next_incremental_id)]
	pub(super) type NextIncrementalId<T: Config> = StorageValue<_, IncrementalIntentId, ValueQuery>;

	#[pallet::storage]
	/// Temporay storage for the best solution, used to exclude worse solutions when tx are submitted.
	#[pallet::getter(fn solution_score)]
	pub(super) type SolutionScore<T: Config> = StorageValue<_, (T::AccountId, u64), OptionQuery>;

	#[pallet::storage]
	/// Flag that indicates if the solution was executed in current block.
	#[pallet::getter(fn solution_executed)]
	pub(super) type SolutionExecuted<T: Config> = StorageValue<_, bool, ValueQuery, ExecDefault>;

	pub struct ExecDefault;

	impl Get<bool> for ExecDefault {
		fn get() -> bool {
			false
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(_n: BlockNumberFor<T>) {
			SolutionScore::<T>::kill();
			SolutionExecuted::<T>::kill();
		}

		fn offchain_worker(block_number: BlockNumberFor<T>) {
			// limit the cases when the offchain worker run
			if sp_io::offchain::is_validator() {
				Self::settle_intents(block_number);
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::submit_intent())] //TODO: should probably include length of on_success/on_failure calls too
		pub fn submit_intent(origin: OriginFor<T>, intent: Intent<T::AccountId, T::AssetId>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let now = T::TimestampProvider::now();
			ensure!(intent.deadline > now, Error::<T>::InvalidDeadline);
			ensure!(
				intent.deadline < (now.saturating_add(T::MaxAllowedIntentDuration::get())),
				Error::<T>::InvalidDeadline
			);

			ensure!(intent.swap.amount_in > Balance::zero(), Error::<T>::InvalidIntent);
			ensure!(intent.swap.amount_out > Balance::zero(), Error::<T>::InvalidIntent);
			ensure!(intent.swap.asset_in != intent.swap.asset_out, Error::<T>::InvalidIntent);
			ensure!(intent.swap.asset_out != T::HubAssetId::get(), Error::<T>::InvalidIntent);

			T::ReservableCurrency::reserve_named(
				&T::NamedReserveId::get(),
				intent.swap.asset_in,
				&who,
				intent.swap.amount_in,
			)?;

			let incremental_id = Self::get_next_incremental_id().ok_or(Error::<T>::IntendIdsExhausted)?;
			let intent_id = Self::get_intent_id(intent.deadline, incremental_id);

			Intents::<T>::insert(intent_id, &intent);

			Self::deposit_event(Event::IntentSubmitted(intent_id, intent));

			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight( {
			let mut w = T::WeightInfo::submit_solution();
			let intent_count = intents.len() as u64;
			let transfer_weight = T::Weigher::transfer_weight() * intent_count * 2; // transfer in and out
			w.saturating_accrue(transfer_weight);
			for instruction in trades.iter() {
				match instruction {
					TradeInstruction::SwapExactIn { route, .. } => {
						w.saturating_accrue(T::Weigher::sell_weight(route.to_vec()));
					},
					TradeInstruction::SwapExactOut { route, .. } => {
						w.saturating_accrue(T::Weigher::buy_weight(route.to_vec()));
					}
				}
			}
			w
		})]
		pub fn submit_solution(
			origin: OriginFor<T>,
			intents: BoundedResolvedIntents,
			trades: BoundedTrades<T::AssetId>,
			score: u64,
			block: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// check if the solution was already executed in this block
			// This is to prevent multiple solutions to be executed in the same block.
			// Although it should be handled by the tx validation, it is better to have it here too.
			// So we dont slash the user for the tx that should have been rejected.
			ensure!(!SolutionExecuted::<T>::get(), Error::<T>::AlreadyExecuted);

			// double-check the target block, although it should be done in the tx validation
			ensure!(
				block == T::BlockNumberProvider::current_block_number(),
				Error::<T>::InvalidBlockNumber
			);

			match ICEEngine::<T>::prepare_solution(intents, trades, score) {
				Ok(solution) => {
					ICEEngine::<T>::execute_solution(solution)?;
					Self::clear_expired_intents();
					Self::deposit_event(Event::SolutionExecuted { who });
					SolutionExecuted::<T>::set(true);
				}
				Err(e) => {
					return Err(e);
				}
			}
			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight( {
			let mut w = T::WeightInfo::submit_solution();
			let intent_count = intents.len() as u64;
			let transfer_weight = T::Weigher::transfer_weight() * intent_count * 2; // transfer in and out
			w.saturating_accrue(transfer_weight);
			for instruction in trades.iter() {
			match instruction {
					TradeInstruction::SwapExactIn { route, .. } => {
						w.saturating_accrue(T::Weigher::sell_weight(route.to_vec()));
					},
					TradeInstruction::SwapExactOut { route, .. } => {
						w.saturating_accrue(T::Weigher::buy_weight(route.to_vec()));
					}
				}
			}
			w
		})]
		pub fn propose_solution(
			origin: OriginFor<T>,
			intents: BoundedResolvedIntents,
			trades: BoundedTrades<T::AssetId>,
			score: u64,
			block: BlockNumberFor<T>,
		) -> DispatchResult {
			ensure_none(origin)?;

			// check if the solution was already executed in this block
			// This is to prevent multiple solutions to be executed in the same block.
			// Although it should be handled by the tx validation, it is better to have it here too.
			// So we dont slash the user for the tx that should have been rejected.
			ensure!(!SolutionExecuted::<T>::get(), Error::<T>::AlreadyExecuted);

			// double-check the target block, although it should be done in the tx validation
			ensure!(
				block == T::BlockNumberProvider::current_block_number(),
				Error::<T>::InvalidBlockNumber
			);

			match ICEEngine::<T>::prepare_solution(intents, trades, score) {
				Ok(solution) => {
					ICEEngine::<T>::execute_solution(solution)?;
					Self::clear_expired_intents();
					//Self::deposit_event(Event::SolutionExecuted { who });
					SolutionExecuted::<T>::set(true);
				}
				Err(e) => {
					return Err(e);
				}
			}
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Holding account
	pub fn holding_account() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	pub fn get_intent_id(deadline: Moment, increment: IncrementalIntentId) -> IntentId {
		(deadline as u128) << 64 | increment as u128
	}

	pub(crate) fn get_next_incremental_id() -> Option<IncrementalIntentId> {
		NextIncrementalId::<T>::mutate(|id| -> Option<IncrementalIntentId> {
			let current_id = *id;
			*id = id.checked_add(1)?;
			Some(current_id)
		})
	}

	pub fn validate_submission(who: &T::AccountId, score: u64, block: BlockNumberFor<T>) -> bool {
		if block != T::BlockNumberProvider::current_block_number() {
			return false;
		}

		if let Some((from, current_score)) = SolutionScore::<T>::get() {
			if score > current_score {
				SolutionScore::<T>::put((who, score));
			}
			if from == *who {
				true
			} else {
				score > current_score
			}
		} else {
			SolutionScore::<T>::put((who, score));
			true
		}
	}

	fn clear_expired_intents() {
		//TODO: perhaps better way to do this is to use a priority queue/ordered list or something.
		let now = T::TimestampProvider::now();
		let mut to_remove = Vec::new();
		for (intent_id, intent) in Intents::<T>::iter() {
			if intent.deadline < now {
				to_remove.push((intent_id, intent));
			}
		}

		for (intent_id, intent) in to_remove {
			let remainder = T::ReservableCurrency::unreserve_named(
				&T::NamedReserveId::get(),
				intent.swap.asset_in,
				&intent.who,
				intent.swap.amount_in,
			); //TODO: add test
			debug_assert!(remainder.is_zero());
			Intents::<T>::remove(intent_id);
		}
	}

	fn ensure_proposal_bond(who: &T::AccountId) -> bool {
		let required_bond = T::ProposalBond::get();
		let balance =
			T::Currency::reducible_balance(T::NativeAssetId::get(), who, Preservation::Protect, Fortitude::Force); //TODO: check params
		balance > required_bond
	}

	fn slash_bond(who: &T::AccountId) -> Result<Balance, DispatchError> {
		T::Currency::transfer(
			T::NativeAssetId::get(),
			who,
			&T::SlashReceiver::get(),
			T::ProposalBond::get(),
			Preservation::Expendable,
		)
	}

	fn settle_intents(block_number: BlockNumberFor<T>) {
		let lock_expiration = Duration::from_millis(LOCK_TIMEOUT_EXPIRATION);
		let mut lock =
			StorageLock::<'_, sp_runtime::offchain::storage_lock::Time>::with_deadline(SOLVER_LOCK, lock_expiration);

		if let Ok(_guard) = lock.try_lock() {
			// Get list of current intents,
			let mut intents: Vec<(IntentId, Intent<T::AccountId, T::AssetId>)> = Intents::<T>::iter().collect();
			intents.sort_by_key(|(_, intent)| intent.deadline);

			// Retain non-expired intents
			let now = T::TimestampProvider::now();
			intents.retain(|(_, intent)| intent.deadline > now);

			// Compute solution using solver
			let Ok(solution) = T::Solver::solve(intents) else {
				//TODO: log error
				return;
			};
		};
	}

	pub fn calculate_trades_and_score(
		resolved_intents: &[ResolvedIntent],
	) -> Result<(Vec<TradeInstruction<T::AssetId>>, u64), ()> {
		let mut amounts_in: BTreeMap<T::AssetId, Balance> = BTreeMap::new();
		let mut amounts_out: BTreeMap<T::AssetId, Balance> = BTreeMap::new();

		for resolved_intent in resolved_intents.iter() {
			let intent = Intents::<T>::get(resolved_intent.intent_id).ok_or(())?;
			amounts_in
				.entry(intent.swap.asset_in)
				.and_modify(|e| *e += resolved_intent.amount_in)
				.or_insert(resolved_intent.amount_in);
			amounts_out
				.entry(intent.swap.asset_out)
				.and_modify(|e| *e += resolved_intent.amount_out)
				.or_insert(resolved_intent.amount_out);
		}

		let mut lrna_aquired = 0u128;

		let mut matched_amounts = Vec::new();
		let mut trades_instructions = Vec::new();

		// Sell all for lrna
		for (asset_id, amount) in amounts_in.iter() {
			let amount_out = *amounts_out.get(asset_id).unwrap_or(&0u128);

			matched_amounts.push((*asset_id, (*amount).min(amount_out)));

			if *amount > amount_out {
				let route = T::RoutingSupport::get_route(*asset_id, T::HubAssetId::get());
				let diff = amount.saturating_sub(amount_out);

				let lrna_bought = T::RoutingSupport::calculate_amount_out(&route, diff)?;
				lrna_aquired.saturating_accrue(lrna_bought);
				trades_instructions.push(TradeInstruction::SwapExactIn {
					asset_in: *asset_id,
					asset_out: T::HubAssetId::get(),
					amount_in: amount.saturating_sub(amount_out), //Swap only difference
					amount_out: lrna_bought,
					route: BoundedRoute::try_from(route).unwrap(),
				});
			}
		}

		let mut lrna_sold = 0u128;

		for (asset_id, amount) in amounts_out {
			let amount_in = *amounts_in.get(&asset_id).unwrap_or(&0u128);

			if amount > amount_in {
				let route = T::RoutingSupport::get_route(T::HubAssetId::get(), asset_id);
				let diff = amount.saturating_sub(amount_in);
				let lrna_in = T::RoutingSupport::calculate_amount_in(&route, diff)?;
				lrna_sold.saturating_accrue(lrna_in);
				trades_instructions.push(TradeInstruction::SwapExactOut {
					asset_in: T::HubAssetId::get(),
					asset_out: asset_id,
					amount_in: lrna_in,
					amount_out: amount.saturating_sub(amount_in), //Swap only difference
					route: BoundedRoute::try_from(route).unwrap(),
				});
			}
		}
		assert!(
			lrna_aquired >= lrna_sold,
			"lrna_aquired < lrna_sold ({} < {})",
			lrna_aquired,
			lrna_sold
		);

		let mut score = resolved_intents.len() as u128 * 1_000_000_000_000;
		for (asset_id, amount) in matched_amounts {
			let price = T::RoutingSupport::hub_asset_price(asset_id)?;
			let h = multiply_by_rational_with_rounding(amount, price.n, price.d, sp_runtime::Rounding::Up).unwrap();
			score.saturating_accrue(h);
		}
		let score = (score / 1_000_000) as u64;
		Ok((trades_instructions, score))
	}
}
