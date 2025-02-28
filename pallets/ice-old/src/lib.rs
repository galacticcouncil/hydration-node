//!
#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

pub mod api;
#[cfg(test)]
mod tests;
pub mod traits;
pub mod types;
pub mod validity;
mod weights;

use crate::traits::AmmState;
use crate::traits::Routing;
use crate::types::{
	AssetId, Balance, BoundedResolvedIntents, BoundedTrades, IncrementalIntentId, Instruction, Intent, IntentId,
	Moment, NamedReserveIdentifier, ResolvedIntent, SolutionAmounts, Swap, SwapType, TradeInstructionTransform,
};
use frame_support::pallet_prelude::StorageValue;
use frame_support::pallet_prelude::*;
use frame_support::traits::fungibles::{Inspect, Mutate};
use frame_support::traits::tokens::{Fortitude, Preservation};
use frame_support::traits::{OneSessionHandler, OriginTrait, Time};
use frame_support::{dispatch::DispatchResult, traits::Get};
use frame_support::{Blake2_128Concat, BoundedSlice};
use frame_system::offchain::{AppCrypto, SendTransactionTypes, Signer, SubmitTransaction};
use frame_system::pallet_prelude::*;
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::RouterT;
use orml_traits::NamedMultiReservableCurrency;
pub use pallet::*;
use scale_info::TypeInfo;
use sp_core::offchain::Duration;
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::offchain::storage_lock::StorageLock;
use sp_runtime::traits::Zero;
use sp_runtime::traits::{AccountIdConversion, BlockNumberProvider, Convert};
use sp_runtime::SaturatedConversion;
use sp_runtime::{ArithmeticError, FixedU128, Rounding, RuntimeAppPublic, Saturating};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::prelude::*;
pub use weights::WeightInfo;

pub const SOLVER_LOCK: &[u8] = b"hydradx/ice-old/lock/";
pub const LOCK_TIMEOUT_EXPIRATION: u64 = 5_000; // 5 seconds

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::traits::IceWeightBounds;
	use crate::types::{BoundedResolvedIntents, BoundedTrades};
	use frame_support::traits::fungibles::Mutate;
	use frame_support::PalletId;
	use frame_system::offchain::{CreateSignedTransaction, SendSignedTransaction};
	use hydra_dx_math::ratio::Ratio;
	use hydradx_traits::price::PriceProvider;
	use sp_runtime::traits::{BlockNumberProvider, UniqueSaturatedInto};
	use types::Balance;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config /* + SendTransactionTypes<Call<Self>>*/ + CreateSignedTransaction<Call<Self>> {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The identifier type for an authority.
		type AuthorityId: Member
			+ Parameter
			+ sp_runtime::RuntimeAppPublic
			+ Ord
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		//type AuthorityId: AppCrypto<Self::Public, Self::Signature>;

		/// Native asset Id
		#[pallet::constant]
		type NativeAssetId: Get<AssetId>;

		/// Asset Id of hub asset
		#[pallet::constant]
		type HubAssetId: Get<AssetId>;

		/// Provider for the current timestamp.
		type TimestampProvider: Time<Moment = Moment>;

		/// Maximum deadline for intent in milliseconds.
		#[pallet::constant]
		type MaxAllowedIntentDuration: Get<Moment>;

		/// Block number provider.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		/// TODO: this two currencies could be merged into one, however it would need to implement support in the runtime for this
		type Currency: Mutate<Self::AccountId, AssetId = AssetId, Balance = types::Balance>;

		type ReservableCurrency: NamedMultiReservableCurrency<
			Self::AccountId,
			ReserveIdentifier = NamedReserveIdentifier,
			CurrencyId = AssetId,
			Balance = Balance,
		>;

		type TradeExecutor: RouterT<
			Self::RuntimeOrigin,
			AssetId,
			Balance,
			hydradx_traits::router::Trade<AssetId>,
			hydradx_traits::router::AmountInAndOut<Balance>,
		>;

		type AmmStateProvider: crate::traits::AmmState<AssetId>;

		/// The means of determining a solution's weight.
		type Weigher: IceWeightBounds<Self::RuntimeCall, Vec<hydradx_traits::router::Trade<AssetId>>>;

		/// Price provider
		type PriceProvider: PriceProvider<AssetId, Price = Ratio>;

		type RoutingSupport: Routing<AssetId>;

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

		/// The maximum number of keys that can be added.
		#[pallet::constant]
		type MaxKeys: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New intent was submitted
		IntentSubmitted(IntentId, Intent<T::AccountId>),

		/// Solution was executed
		SolutionExecuted {
			who: T::AccountId,
		},
		Hurray {
			score: u64,
		},
	}

	#[derive(Encode, Decode, Eq, PartialEq, TypeInfo, frame_support::PalletError, RuntimeDebug)]
	pub enum Reason {
		Empty,
		Score,
		IntentNotFound,
		IntentAmount,
		IntentPartialAmount,
		IntentPrice,
	}

	#[pallet::error]
	pub enum Error<T> {
		/// No more intent ids available
		IntendIdsExhausted,

		/// Data too long
		TooLong,

		/// Price is missing in provided solution
		MissingPrice,

		/// Invalid block number
		InvalidBlockNumber,

		/// Invalid deadline
		InvalidDeadline,

		/// Insufficient reserved balance
		InsufficientReservedBalance,

		/// Solution already executed in this block
		AlreadyExecuted,

		/// Invalid intent parameters
		InvalidIntent,

		///
		InvalidSolution(Reason),

		/// Incorrect intent update
		InvalidIntentUpdate,

		/// Error in trading - failed to determine route, calculate trading amounts
		TradingError,

		/// Failed signing
		FailedSigning,

		/// Failed to submit transaction
		SubmitTransaction,
	}

	#[pallet::storage]
	#[pallet::getter(fn get_intent)]
	pub(super) type Intents<T: Config> = StorageMap<_, Blake2_128Concat, IntentId, Intent<T::AccountId>>;

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

	/*
	/// The current set of keys that may submit a solution
	#[pallet::storage]
	pub type Keys<T: Config> = StorageValue<_, WeakBoundedVec<T::AuthorityId, T::MaxKeys>, ValueQuery>;

	 */

	pub struct ExecDefault;

	impl Get<bool> for ExecDefault {
		fn get() -> bool {
			false
		}
	}

	/*
	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub keys: Vec<T::AuthorityId>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			Pallet::<T>::initialize_keys(&self.keys);
		}
	}

	 */

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(_n: BlockNumberFor<T>) {
			SolutionScore::<T>::kill();
			SolutionExecuted::<T>::kill();
		}

		fn offchain_worker(block_number: BlockNumberFor<T>) {
			log::error!("Running ice-old offchain worker");
			let lock_expiration = Duration::from_millis(LOCK_TIMEOUT_EXPIRATION);
			let mut lock = StorageLock::<'_, sp_runtime::offchain::storage_lock::Time>::with_deadline(
				SOLVER_LOCK,
				lock_expiration,
			);

			if let Ok(_guard) = lock.try_lock() {
				if sp_io::offchain::is_validator() {
					let intents: Vec<crate::api::IntentRepr> = Self::get_valid_intents()
						.into_iter()
						.map(|x| {
							let mut info: crate::api::IntentRepr = x.1.into();
							info.0 = x.0;
							info
						})
						.collect();
					// TODO: change this when ready
					//if intents.len() > 0 {
					if true {
						let data = T::AmmStateProvider::state(|_| true)
							.into_iter()
							.map(|x| x.into())
							.collect();
						log::error!("Getting solution");
						let s = api::ice::get_solution(intents, data);
						log::error!("Solution {:?}", s);
						let score = Self::calculate_score(&s);
						let next_block: BlockNumberFor<T> = block_number.saturating_add(1u32.into());

						/*
						let signer = Signer::<T, T::AuthorityId>::all_accounts();
						if !signer.can_sign() {
							//TODO: handle error
							return;
						}
						let results = signer.send_signed_transaction(|_account| Call::submit_solution {
							intents: BoundedResolvedIntents::truncate_from(s.clone()),
							score,
							block: next_block,
						});
						//TODO: handle result

						 */

						/* with runtimepublicapp
						//just take first one for now
						let a_idx = 0usize;
						let keys = T::AuthorityId::all();
						let key = keys[a_idx].clone();

						//TODO: account?!
						let ac = T::AuthorityId::all().len() as u32;

						let call: pallet::Call<T> = Call::submit_solution{
							intents: BoundedResolvedIntents::truncate_from(s),
							score,
							block: next_block,
						};
						let t = CreateSignedTransaction::<Call<T>>::create_transaction::<T::AuthorityId>(call.into(), key, ac, None).ok_or(Error::FailedSigning).unwrap();
						let r = SubmitTransaction::<T, Call<T>>::submit_transaction(t.0, t.1);

						 */

						//TODO: handle result

						/* Unsigned solution
						let params = (s.clone(), 1u64, next_block);
						let signature = key.sign(&params.encode()).ok_or(Error::<T>::FailedSigning).unwrap();
						let call: pallet::Call<T> = Call::propose_solution {
							intents: BoundedResolvedIntents::truncate_from(s),
							score: 1u64,
							block,
							signature,
							signer: crate::types::Signer::Authority(a_idx as u32)
						};

						log::error!("Sending tx");
						let _ = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into());
						 */

						//let r = SubmitTransaction::<T, Call<T>>::submit_transaction(call.into(), Some(signature));

						//let t = CreateSignedTransaction::<Call<T>>::create_transaction::<T::AuthorityId>(call.into(), key, ac, None).ok_or(Error::FailedSigning).unwrap();

						//let r = SubmitTransaction::<T, Call<T>>::submit_transaction(t.0, t.1);
					}
				}
			};
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::submit_intent())] //TODO: should probably include length of on_success/on_failure calls too
		pub fn submit_intent(origin: OriginFor<T>, intent: Intent<T::AccountId>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(who == intent.who, Error::<T>::InvalidIntent);

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
			//TODO: how do we account weight for trades now ?!! since it is removed
			// probably we can take the worst case scenario - which is one intent - one trade
			// number of resolved intent is known
			// we can consider returning unused weight
			w
		})]
		pub fn submit_solution(
			origin: OriginFor<T>,
			intents: BoundedResolvedIntents,
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

			// double-check again, should be done in tx validation
			ensure!(!intents.is_empty(), Error::<T>::InvalidSolution(Reason::Empty));

			//TODO: hm..clone here is not optimal, do something, bob!
			// TODO: remove trades
			match Self::validate_and_prepare_instructions(intents.clone().to_vec(), BoundedTrades::default(), score) {
				Ok((instructions, amounts)) => {
					Self::execute_instructions(instructions, amounts)?;
					Self::update_intents(intents)?;
					Self::clear_expired_intents(); //TODO: in on finalize!!
					Self::deposit_event(Event::SolutionExecuted { who });
					SolutionExecuted::<T>::set(true);
				}
				Err(e) => {
					return Err(e);
				}
			}
			Ok(())
		}

		/*
		//TODO: same as submit, but unsigned,
		// please merge into one, bob!
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::submit_intent())] //TODO: should probably include length of on_success/on_failure calls too
		pub fn propose_solution(
			origin: OriginFor<T>,
			_intents: BoundedResolvedIntents,
			score: u64,
			_block: BlockNumberFor<T>,
			// since signature verification is done in `validate_unsigned`
			// we can skip doing it here again.
			_signature: <T::AuthorityId as RuntimeAppPublic>::Signature,
			_signer: crate::types::Signer<T::AccountId>,
		) -> DispatchResult {
			ensure_none(origin)?;
			Self::deposit_event(Event::Hurray { score });
			Ok(())
		}

		 */
	}

	/*
	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(_source: TransactionSource, _call: &Self::Call) -> TransactionValidity {
			if let Call::propose_solution { intents, score, block, signature, signer } = _call {

				match signer {
					crate::types::Signer::Authority(idx) => {
						log::error!("Validating!!");
						//let keys = Keys::<T>::get();
						let keys= T::AuthorityId::all();
						let aid = keys.get(*idx as usize).unwrap();

						let params = (intents, score, block).encode();

						let signature_valid = aid.verify(&params, signature);

						if !signature_valid {
							return InvalidTransaction::BadProof.into();
						}
					},
					crate::types::Signer::Account(account) => {
						todo!()
					}
				}

				ValidTransaction::with_tag_prefix("iceice")
					.priority(u64::MAX)
					.longevity(3)
					.propagate(false)
					.and_provides(("solution", score))
					.build()
			} else {
				InvalidTransaction::Call.into()
			}
		}
	}

	 */
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
		//TODO: we could probaly overflow here and starts from 0 when all ids are used
		// it does not matter really as intent id uses dealdine, so it will be unique regardless
		NextIncrementalId::<T>::mutate(|id| -> Option<IncrementalIntentId> {
			let current_id = *id;
			*id = id.checked_add(1)?;
			Some(current_id)
		})
	}

	pub fn validate_submission(
		who: &T::AccountId,
		resolved_intents: &BoundedResolvedIntents,
		score: u64,
		block: BlockNumberFor<T>,
	) -> bool {
		if block != T::BlockNumberProvider::current_block_number() {
			return false;
		}

		if resolved_intents.is_empty() {
			return false;
		}

		if let Some((from, current_score)) = SolutionScore::<T>::get() {
			match score.cmp(&current_score) {
				sp_std::cmp::Ordering::Greater => {
					SolutionScore::<T>::put((who, score));
					true
				}
				sp_std::cmp::Ordering::Less => false,
				sp_std::cmp::Ordering::Equal => {
					// We need to do this, because validate_submission is called multiple times during tx validation
					// and we want to ensure that the tx is valid.
					// Note that one could submit same submit_solution call multiple times, and this would mean that
					// all txs would be valid. However only one would be executed due to SolutionExecuted flag.
					*who == from
				}
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
			);
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

	pub fn get_valid_intents() -> Vec<(IntentId, Intent<T::AccountId>)> {
		let mut intents: Vec<(IntentId, Intent<T::AccountId>)> = Intents::<T>::iter().collect();
		intents.sort_by_key(|(_, intent)| intent.deadline);

		let now = T::TimestampProvider::now();
		intents.retain(|(_, intent)| intent.deadline > now);

		intents
	}

	fn calculate_score(resolved_intent: &[ResolvedIntent]) -> u64 {
		//TODO:
		1u64
	}

	// Calculate score for the solution
	// The score is calculated as follows:
	// 1. For each resolved intent, we add 1_000_000_000_000 to the score
	// 2. For each matched amount, we convert it to hub asset and add it to the score
	// 3. The final score is rounded down by dividing by 1_000_000
	// Parameters:
	// - resolved_intents: number of resolved intents
	// - matched_amounts: list of matched amounts
	fn score_solution(resolved_intents: u128, matched_amounts: Vec<(AssetId, Balance)>) -> Result<u64, DispatchError> {
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

	// Prepare solution for execution
	// 1. Validate resolved intents - ensure price, partials, amounts are correct
	// 2. Build list of transfers in and transfers out
	// 3. Merge with list of trades
	// 4. Calculate matched amount and score the solution
	// 5. Ensure score solution is correct
	fn validate_and_prepare_instructions(
		intents: Vec<ResolvedIntent>,
		trades: BoundedTrades<AssetId>,
		score: u64,
	) -> Result<(Vec<Instruction<T::AccountId, AssetId>>, SolutionAmounts<AssetId>), DispatchError> {
		let mut amounts_in: BTreeMap<AssetId, Balance> = BTreeMap::new();
		let mut amounts_out: BTreeMap<AssetId, Balance> = BTreeMap::new();

		let mut transfers_in: Vec<Instruction<T::AccountId, AssetId>> = Vec::new();
		let mut transfers_out: Vec<Instruction<T::AccountId, AssetId>> = Vec::new();

		for resolved_intent in intents.iter() {
			let intent = Intents::<T>::get(resolved_intent.intent_id)
				.ok_or(Error::<T>::InvalidSolution(Reason::IntentNotFound))?;

			ensure!(
				Self::ensure_intent_price(&intent, resolved_intent),
				Error::<T>::InvalidSolution(Reason::IntentPrice)
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
							Error::<T>::InvalidSolution(Reason::IntentAmount)
						);
					} else {
						ensure!(
							resolved_intent.amount_in == intent.swap.amount_in,
							Error::<T>::InvalidSolution(Reason::IntentPartialAmount)
						);
						ensure!(
							resolved_intent.amount_out >= intent.swap.amount_out,
							Error::<T>::InvalidSolution(Reason::IntentPartialAmount)
						);
					}
				}
				SwapType::ExactOut => {
					if is_partial {
						ensure!(
							resolved_intent.amount_out <= intent.swap.amount_out,
							Error::<T>::InvalidSolution(Reason::IntentAmount)
						);
					} else {
						ensure!(
							resolved_intent.amount_out == intent.swap.amount_out,
							Error::<T>::InvalidSolution(Reason::IntentPartialAmount)
						);
						ensure!(
							resolved_intent.amount_in <= intent.swap.amount_in,
							Error::<T>::InvalidSolution(Reason::IntentPartialAmount)
						);
					}
				}
			}
		}

		let mut matched_amounts = Vec::new();
		for (asset_id, amount) in amounts_in.iter() {
			let amount_out = amounts_out.get(asset_id).unwrap_or(&0u128);
			let matched_amount = (*amount).min(*amount_out);
			if matched_amount > 0u128 {
				matched_amounts.push((*asset_id, matched_amount));
			}
		}

		let mut instructions = Vec::new();

		instructions.extend(transfers_in);
		instructions.extend(TradeInstructionTransform::convert(trades));
		instructions.extend(transfers_out);

		let calculated_score = Self::score_solution(intents.len() as u128, matched_amounts)?;
		ensure!(calculated_score == score, Error::<T>::InvalidSolution(Reason::Score));

		Ok((
			instructions,
			SolutionAmounts {
				amounts_in,
				amounts_out,
			},
		))
	}

	fn ensure_intent_price(intent: &Intent<T::AccountId>, resolved_intent: &ResolvedIntent) -> bool {
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

		let diff = if realized < expected {
			expected - realized
		} else {
			realized - expected
		};

		diff <= FixedU128::from_rational(1, 1000)
	}

	fn execute_instructions(
		instructions: Vec<Instruction<T::AccountId, AssetId>>,
		amounts: SolutionAmounts<AssetId>,
	) -> Result<(), DispatchError> {
		let holding_account = crate::Pallet::<T>::holding_account();

		// iterate and act only on TrensferIn instruction
		for instruction in instructions.iter() {
			match instruction {
				Instruction::TransferIn { who, asset_id, amount } => {
					let r = T::ReservableCurrency::unreserve_named(&T::NamedReserveId::get(), *asset_id, &who, *amount);
					ensure!(r == Balance::zero(), crate::Error::<T>::InsufficientReservedBalance);
					T::Currency::transfer(*asset_id, &who, &holding_account, *amount, Preservation::Expendable)?;
				}
				_ => {}
			}
		}

		// now do the trades
		Self::do_trades(amounts.amounts_in, amounts.amounts_out)?;

		for instruction in instructions.iter() {
			match instruction {
				Instruction::TransferOut { who, asset_id, amount } => {
					T::Currency::transfer(*asset_id, &holding_account, &who, *amount, Preservation::Expendable)?;
				}
				_ => {}
			}
		}

		Ok(())
	}

	fn update_intents(resolved_intents: BoundedResolvedIntents) -> DispatchResult {
		for resolved_intent in resolved_intents.iter() {
			let Some(intent) = Intents::<T>::take(resolved_intent.intent_id) else {
				//we can ignore this case, should not happen
				defensive!("Updating intents - intent not found");
				continue;
			};

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
				defensive!("Partially resolved intent that should not be partial");
				return Err(Error::<T>::InvalidIntentUpdate.into());
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

	fn do_trades(amounts_in: BTreeMap<AssetId, Balance>, amounts_out: BTreeMap<AssetId, Balance>) -> DispatchResult {
		let mut amounts_in: BTreeMap<AssetId, Balance> = amounts_in;

		let mut matched_amounts = Vec::new();

		let mut delta_in: BTreeMap<AssetId, Balance> = BTreeMap::new();
		let mut delta_out: BTreeMap<AssetId, Balance> = BTreeMap::new();

		// Calculate deltas to trade
		for (asset_id, amount_out) in amounts_out.into_iter() {
			if let Some((_, amount_in)) = amounts_in.remove_entry(&asset_id) {
				if amount_out == amount_in {
					// nothing to trade here, all matched
					matched_amounts.push((asset_id, amount_out));
				} else if amount_out > amount_in {
					// there is something left to buy
					matched_amounts.push((asset_id, amount_in));
					delta_out.insert(asset_id, amount_out - amount_in);
				} else {
					// there is something left to sell
					matched_amounts.push((asset_id, amount_out));
					delta_in.insert(asset_id, amount_in - amount_out);
				}
			} else {
				// there is no sell of this asset, only buy
				delta_out.insert(asset_id, amount_out);
			}
		}

		for (asset_id, amount_in) in amounts_in.into_iter() {
			delta_in.insert(asset_id, amount_in);
		}

		let holding_account = crate::Pallet::<T>::holding_account();

		loop {
			let Some((asset_out, mut amount_out)) = delta_out.pop_first() else {
				break;
			};
			for (asset_in, amount_in) in delta_in.iter_mut() {
				if *amount_in == 0u128 {
					continue;
				}
				let route = T::RoutingSupport::get_route(*asset_in, asset_out);
				// TODO: when time comes, we would need more sophisticated route selection due to multiple pools

				// Calculate the amount we can buy with the amount in we have
				let possible_out_amount = T::RoutingSupport::calculate_amount_out(&route, *amount_in)
					.map_err(|_| Error::<T>::TradingError)?;

				if possible_out_amount >= amount_out {
					// do exact buy
					let a_in = T::RoutingSupport::calculate_amount_in(&route, amount_out)
						.map_err(|_| Error::<T>::TradingError)?;

					if a_in > *amount_in {
						// this is a bug!
						defensive!("Trading - amount in is less than expected. Bug!");
						return Err(Error::<T>::TradingError.into());
					}

					let origin = T::RuntimeOrigin::signed(holding_account.clone());
					T::TradeExecutor::buy(origin, *asset_in, asset_out, amount_out, a_in, route.to_vec())?;

					*amount_in -= a_in; // this is safe, because of the condition
					amount_out = 0u128;
					//after this, we sorted the asset_out, we can move one
					break;
				} else {
					// do max sell
					let origin = T::RuntimeOrigin::signed(holding_account.clone());
					T::TradeExecutor::sell(
						origin,
						*asset_in,
						asset_out,
						*amount_in,
						possible_out_amount,
						route.to_vec(),
					)?;

					*amount_in = 0u128;
					amount_out -= possible_out_amount; //this is safe, because of the condition
					                    //after this, we need another asset_in to buy the rest
				}
			}

			// ensure we sorted asset out before moving on
			//TODO: is this really needed? rethink!
			debug_assert!(amount_out == 0u128);
		}

		Ok(())
	}

	/*
	fn initialize_keys(keys: &[T::AuthorityId]) {
		if !keys.is_empty() {
			assert!(Keys::<T>::get().is_empty(), "Keys are already initialized!");
			let bounded_keys = <BoundedSlice<'_, _, T::MaxKeys>>::try_from(keys)
				.expect("More than the maximum number of keys provided");
			Keys::<T>::put(bounded_keys);
		}
	}

	fn local_authority_keys() -> impl Iterator<Item = (u32, T::AuthorityId)> {
		let authorities = Keys::<T>::get();
		log::error!("storage {:?}", authorities.len());

		// local keystore
		// All public (+private) keys currently in the local keystore.
		let mut local_keys = T::AuthorityId::all();
		log::error!("Local keys {:?}", local_keys.len());

		local_keys.sort();

		authorities
			.into_iter()
			.enumerate()
			.filter_map(move |(index, authority)| {
				local_keys
					.binary_search(&authority)
					.ok()
					.map(|location| (index as u32, local_keys[location].clone()))
			})
	}

	 */
}

/*
impl<T: Config> sp_runtime::BoundToRuntimeAppPublic for Pallet<T> {
	type Public = T::AuthorityId;
}

impl<T: Config> OneSessionHandler<T::AccountId> for Pallet<T> {
	type Key = T::AuthorityId;

	fn on_genesis_session<'a, I: 'a>(validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, Self::Key)>,
	{
		let keys = validators.map(|x| x.1).collect::<Vec<_>>();
		Self::initialize_keys(&keys);
	}

	fn on_new_session<'a, I: 'a>(changed: bool, validators: I, queued_validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, Self::Key)>,
	{
		// Remember who the authorities are for the new session.
		let keys = validators.map(|x| x.1).collect::<Vec<_>>();
		let bounded_keys = WeakBoundedVec::<_, T::MaxKeys>::force_from(
			keys,
			Some(
				"Warning: The session has more keys than expected. \
				  A runtime configuration adjustment may be needed.",
			),
		);
		Keys::<T>::put(bounded_keys);
	}

	fn on_disabled(_validator_index: u32) {}
}

 */
