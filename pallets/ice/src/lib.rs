#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

pub mod api;
#[cfg(test)]
mod tests;
mod traits;
pub mod types;
mod weights;

use crate::api::{into_intent_repr, into_pool_data_repr, DataRepr, IntentRepr};
use frame_support::pallet_prelude::*;
use frame_support::traits::fungibles::Mutate;
use frame_support::traits::tokens::Preservation;
use frame_support::PalletId;
use frame_support::{dispatch::DispatchResult, require_transactional, traits::Get};
use frame_system::offchain::{AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer};
use frame_system::pallet_prelude::*;
use hydra_dx_math::ratio::Ratio;
use hydradx_traits::ice::AmmState;
use hydradx_traits::price::PriceProvider;
pub use pallet::*;
use pallet_intent::types::{BoundedResolvedIntents, Intent, IntentId, ResolvedIntent, SwapType};
use sp_core::offchain::KeyTypeId;
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::{ArithmeticError, FixedU128, Rounding, SaturatedConversion, Saturating};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec;
use sp_std::vec::Vec;
use traits::Trader;
use types::{AssetId, Balance, Reason, Solution};
pub use weights::WeightInfo;

// Some useful constants
const LOG_TARGET: &str = "runtime::ice-old";
//const ICE_SOLVER_LOCK: &[u8] = b"hydration/ice-old/lock/";
//const ICE_SOLVER_LOCK_TIMEOUT: u64 = 5_000; // 5 seconds

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"ICEK");

pub mod crypto {
	use super::KEY_TYPE;
	use sp_core::sr25519::Signature as Sr25519Signature;
	use sp_runtime::{
		app_crypto::{app_crypto, sr25519},
		traits::Verify,
		MultiSignature, MultiSigner,
	};
	app_crypto!(sr25519, KEY_TYPE);

	pub struct IceKeyId;

	impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for IceKeyId {
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}

	// implemented for mock runtime in test
	impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature> for IceKeyId {
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::traits::fungibles::Mutate;
	use pallet_intent::types::BoundedResolvedIntents;
	use sp_runtime::traits::BlockNumberProvider;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	/// Temporary storage for the best solution, used to exclude worse solutions when tx is submitted.
	#[pallet::getter(fn solution_score)]
	pub(super) type SolutionScore<T: Config> = StorageValue<_, (T::AccountId, u64), OptionQuery>;

	#[pallet::storage]
	/// Flag that indicates if the solution was executed in current block.
	#[pallet::getter(fn solution_executed)]
	pub(super) type SolutionExecuted<T: Config> = StorageValue<_, bool, ValueQuery, ExecDefault>;

	// Default executed flag is false.
	pub struct ExecDefault;
	impl Get<bool> for ExecDefault {
		fn get() -> bool {
			false
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_intent::Config + CreateSignedTransaction<Call<Self>> {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The identifier type for an offchain worker.
		type AuthorityId: AppCrypto<Self::Public, Self::Signature>;

		/// Pallet id - used to create holding account
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Block number provider.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		/// Transfer support
		type Currency: Mutate<Self::AccountId, AssetId = AssetId, Balance = Balance>;

		/// Price provider
		type PriceProvider: PriceProvider<AssetId, Price = Ratio>;

		/// Trader support - used to execute trades given assets and amounts in and out
		type Trader: Trader<Self::AccountId, Outcome = ()>;

		/// Provider of states of all AMM pools
		type AmmStateProvider: AmmState<AssetId>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Solution has been executed.
		Executed { who: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid block number
		InvalidBlockNumber,

		/// Solution already executed in this block
		AlreadyExecuted,

		/// Submitted solution is invalid due to the reason.
		InvalidSolution(Reason),

		/// Failed to retrieve asset price
		MissingPrice,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::submit_solution())]
		pub fn submit_solution(
			origin: OriginFor<T>,
			intents: BoundedResolvedIntents,
			score: u64,
			valid_for_block: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// check if the solution was already executed in this block
			// This is to prevent multiple solutions to be executed in the same block.
			// Although it should be handled by the tx validation, it is better to have it here too.
			// So we dont slash the user for the tx that should have been rejected.
			ensure!(!SolutionExecuted::<T>::get(), Error::<T>::AlreadyExecuted);

			// double-check the target block, although it should be done in the tx validation
			ensure!(
				valid_for_block == T::BlockNumberProvider::current_block_number(),
				Error::<T>::InvalidBlockNumber
			);

			// double-check again, should be done in tx validation
			ensure!(!intents.is_empty(), Error::<T>::InvalidSolution(Reason::Empty));

			let solution = Self::prepare_solution(&intents)?;

			Self::validate_solution_score(&solution, intents.len() as u128, score)?;
			Self::execute_solution(solution)?;
			Self::update_resolved_intents(intents)?;
			Self::clear_expired_intents()?;

			Self::deposit_event(Event::Executed { who });

			SolutionExecuted::<T>::set(true);
			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(_n: BlockNumberFor<T>) {
			SolutionScore::<T>::kill();
			SolutionExecuted::<T>::kill();
		}

		fn offchain_worker(block_number: BlockNumberFor<T>) {
			log::info!(target: LOG_TARGET, "offchain_worker: block_number: {:?}", block_number);
			// Only validator
			if !sp_io::offchain::is_validator() {
				return;
			}

			//TODO: consider lock guard with timeout here!

			let signer = Signer::<T, T::AuthorityId>::all_accounts();
			if !signer.can_sign() {
				log::error!(target: LOG_TARGET, "No local accounts available. Consider adding one via `author_insertKey` RPC.");
				return;
			}

			let call = Self::run(block_number, |i, d| Some(api::ice::get_solution(i, d)));

			if let Some(c) = call {
				let results = signer.send_signed_transaction(|_account| c.clone());
				for (_account_id, result) in results.into_iter() {
					if result.is_err() {
						log::error!(target: LOG_TARGET, "Unable to submit transaction: :{:?}", result);
					}
				}
			}
		}
	}
}

// PALLET PUBLIC API
impl<T: Config> Pallet<T> {
	pub fn holding_account() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	/// Prepare solution for execution given resolved intents:
	/// 1. Check if intent exists
	/// 2. Check if intent price is correct
	/// 3. Ensure intent amounts are correct
	/// 4. Construct list of transfers
	fn prepare_solution(resolved_intents: &[ResolvedIntent]) -> Result<Solution<T::AccountId>, DispatchError> {
		let mut amounts: BTreeMap<AssetId, (Balance, Balance)> = BTreeMap::new();
		let mut transfers_in: Vec<(T::AccountId, AssetId, Balance)> = Vec::new();
		let mut transfers_out: Vec<(T::AccountId, AssetId, Balance)> = Vec::new();

		for resolved_intent in resolved_intents.iter() {
			let intent = pallet_intent::Pallet::<T>::get_intent(resolved_intent.intent_id)
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

			amounts
				.entry(asset_in)
				.and_modify(|(v_in, _)| *v_in = v_in.saturating_add(resolved_amount_in))
				.or_insert((resolved_amount_in, 0u128));
			amounts
				.entry(asset_out)
				.and_modify(|(_, v_out)| *v_out = v_out.saturating_add(resolved_amount_out))
				.or_insert((0u128, resolved_amount_out));

			transfers_in.push((intent.who.clone(), asset_in, resolved_amount_in));
			transfers_out.push((intent.who.clone(), asset_out, resolved_amount_out));

			// Ensure the amounts does not exceed the intent amounts
			match intent.swap.swap_type {
				SwapType::ExactIn => {
					if is_partial {
						ensure!(
							resolved_intent.amount_in <= intent.swap.amount_in,
							Error::<T>::InvalidSolution(Reason::IntentPartialAmount)
						);
					} else {
						ensure!(
							resolved_intent.amount_in == intent.swap.amount_in,
							Error::<T>::InvalidSolution(Reason::IntentAmount)
						);
						ensure!(
							resolved_intent.amount_out >= intent.swap.amount_out,
							Error::<T>::InvalidSolution(Reason::IntentAmount)
						);
					}
				}
				SwapType::ExactOut => {
					if is_partial {
						ensure!(
							resolved_intent.amount_out <= intent.swap.amount_out,
							Error::<T>::InvalidSolution(Reason::IntentPartialAmount)
						);
					} else {
						ensure!(
							resolved_intent.amount_out == intent.swap.amount_out,
							Error::<T>::InvalidSolution(Reason::IntentAmount)
						);
						ensure!(
							resolved_intent.amount_in <= intent.swap.amount_in,
							Error::<T>::InvalidSolution(Reason::IntentAmount)
						);
					}
				}
			}
		}
		Ok(Solution {
			transfers_in,
			transfers_out,
			amounts,
		})
	}

	/// Calculate score of provided solution and compare to given score.
	/// Solution score is calculated as follows:
	/// 1. Match trading amounts
	/// 2. Convert difference to Hub Asset
	/// 3. Sum all differences
	/// 4. add 1 UNIT of hub asset for each intent
	/// 5. Divide by 1_000_000 to exclude potential rounding errors
	fn validate_solution_score(
		solution: &Solution<T::AccountId>,
		resolved_intents_count: u128,
		score: u64,
	) -> DispatchResult {
		let mut hub_amount = resolved_intents_count * 1_000_000_000_000u128;

		for (asset_id, (amount_in, amount_out)) in solution.amounts.iter() {
			let matched_amount = (*amount_in).min(*amount_out);
			if matched_amount > 0u128 {
				let price = T::PriceProvider::get_price(<T as pallet_intent::Config>::HubAssetId::get(), *asset_id)
					.ok_or(Error::<T>::MissingPrice)?;
				let converted = multiply_by_rational_with_rounding(matched_amount, price.n, price.d, Rounding::Down)
					.ok_or(ArithmeticError::Overflow)?;
				hub_amount.saturating_accrue(converted);
			}
		}

		let calculated_score = hub_amount / 1_000_000u128;

		ensure!(
			calculated_score == score as u128,
			Error::<T>::InvalidSolution(Reason::Score)
		);
		Ok(())
	}

	fn calculate_score(amounts: &[(AssetId, (Balance, Balance))], resolved_count: u128) -> Result<u64, DispatchError> {
		let mut hub_amount = resolved_count * 1_000_000_000_000u128;

		for (asset_id, (amount_in, amount_out)) in amounts.iter() {
			let matched_amount = (*amount_in).min(*amount_out);
			if matched_amount > 0u128 {
				let price = T::PriceProvider::get_price(<T as pallet_intent::Config>::HubAssetId::get(), *asset_id)
					.ok_or(Error::<T>::MissingPrice)?;
				let converted = multiply_by_rational_with_rounding(matched_amount, price.n, price.d, Rounding::Down)
					.ok_or(ArithmeticError::Overflow)?;
				hub_amount.saturating_accrue(converted);
			}
		}

		let calculated_score = hub_amount / 1_000_000u128;
		Ok(calculated_score as u64)
	}

	#[require_transactional]
	fn execute_solution(solution: Solution<T::AccountId>) -> DispatchResult {
		let holding_account = crate::Pallet::<T>::holding_account();

		for (who, asset_id, amount_in) in solution.transfers_in.iter() {
			T::Currency::transfer(*asset_id, who, &holding_account, *amount_in, Preservation::Expendable)?;
		}

		// now do the trades
		let trade_amounts: Vec<(AssetId, (Balance, Balance))> = solution
			.amounts
			.iter()
			.map(|(asset_id, (amount_in, amount_out))| (*asset_id, (*amount_in, *amount_out)))
			.collect();
		T::Trader::trade(holding_account.clone(), trade_amounts)?;

		for (who, asset_id, amount_out) in solution.transfers_out.iter() {
			T::Currency::transfer(*asset_id, &holding_account, who, *amount_out, Preservation::Expendable)?;
		}

		Ok(())
	}

	#[require_transactional]
	fn update_resolved_intents(resolved: BoundedResolvedIntents) -> DispatchResult {
		for intent in resolved {
			pallet_intent::Pallet::<T>::resolve_intent(intent)?;
		}
		Ok(())
	}

	#[require_transactional]
	fn clear_expired_intents() -> DispatchResult {
		pallet_intent::Pallet::<T>::clear_expired_intents()
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
}

// OFFCHAIN WORKER SUPPORT
impl<T: Config> Pallet<T> {
	pub fn run<F>(block_no: BlockNumberFor<T>, solve: F) -> Option<Call<T>>
	where
		F: FnOnce(Vec<IntentRepr>, Vec<DataRepr>) -> Option<Vec<ResolvedIntent>>,
	{
		//TODO: ensure max intents / resolved intents somehow

		// 1. Get valid intents
		let intents = Self::get_valid_intents();
		let pool_data = T::AmmStateProvider::state(|_| true);

		// 2. Prepare data
		let intents: Vec<api::IntentRepr> = intents.into_iter().map(|intent| into_intent_repr(intent)).collect();
		let data = pool_data.into_iter().map(|d| into_pool_data_repr(d)).collect();

		// 2. Call solver
		let resolved_intents = solve(intents, data)?;

		// 3. calculate score
		//TODO: retrieving intent again -  why, bob, why?
		let mut amounts: BTreeMap<AssetId, (Balance, Balance)> = BTreeMap::new();
		for resolved in resolved_intents.iter() {
			let intent = pallet_intent::Pallet::<T>::get_intent(resolved.intent_id).unwrap();
			amounts
				.entry(intent.swap.asset_in)
				.and_modify(|(v_in, _)| *v_in = v_in.saturating_add(resolved.amount_in))
				.or_insert((resolved.amount_in, 0u128));
			amounts
				.entry(intent.swap.asset_out)
				.and_modify(|(_, v_out)| *v_out = v_out.saturating_add(resolved.amount_out))
				.or_insert((0u128, resolved.amount_out));
		}
		let amounts: Vec<(AssetId, (Balance, Balance))> = amounts.into_iter().collect();
		let score = Self::calculate_score(&amounts, resolved_intents.len() as u128).ok()?;

		Some(Call::submit_solution {
			intents: BoundedResolvedIntents::truncate_from(resolved_intents),
			score,
			valid_for_block: block_no.saturating_add(1u32.saturated_into()), // next block
		})
	}

	fn get_valid_intents() -> Vec<(IntentId, Intent<T::AccountId>)> {
		pallet_intent::Pallet::<T>::get_valid_intents()
	}
}
