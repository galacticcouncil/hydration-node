#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::comparison_chain)]

#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::unnecessary_wraps)]

use frame_support::{dispatch, ensure};
use frame_system::{self as system, ensure_signed};

use codec::Encode;
use sp_std::vec::Vec;

use primitives::{
	asset::AssetPair,
	traits::{Resolver, AMM},
	Amount, AssetId, Balance, ExchangeIntention, IntentionType, MIN_TRADING_LIMIT,
};
use sp_std::borrow::ToOwned;

use orml_traits::{MultiCurrency, MultiCurrencyExtended, MultiReservableCurrency};

use direct::{DirectTradeData, Transfer};
use frame_support::weights::Weight;
use primitives::traits::AMMTransfer;

use frame_support::sp_runtime::offchain::storage_lock::BlockNumberProvider;
use frame_support::sp_runtime::traits::Hash;

#[cfg(test)]
mod mock;

pub mod weights;

use weights::WeightInfo;

mod direct;
#[cfg(test)]
mod tests;

/// Intention alias
type IntentionId<T> = <T as system::Config>::Hash;
pub type Intention<T> = ExchangeIntention<<T as system::Config>::AccountId, Balance, IntentionId<T>>;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		/// Finalize and resolve all registered intentions.
		/// Group/match intentions which can be directly traded.
		fn on_finalize(_n: T::BlockNumber) {
			for ((asset_1, asset_2), count) in ExchangeAssetsIntentionCount::<T>::iter() {
				// If no intention registered for asset1/2, move onto next one
				if count == 0u32 {
					continue;
				}
				let pair = AssetPair {
					asset_in: asset_1,
					asset_out: asset_2,
				};

				let pair_account = T::AMMPool::get_pair_id(pair);

				let asset_a_ins = <ExchangeAssetsIntentions<T>>::get((asset_2, asset_1));
				let asset_b_ins = <ExchangeAssetsIntentions<T>>::get((asset_1, asset_2));

				//TODO: we can short circuit here if nothing in asset_b_sells and just resolve asset_a sells.

				Self::process_exchange_intentions(&pair_account, &asset_a_ins, &asset_b_ins);
			}

			ExchangeAssetsIntentionCount::<T>::remove_all();
			ExchangeAssetsIntentions::<T>::remove_all();
		}

		fn on_initialize(_n: T::BlockNumber) -> Weight {
			T::WeightInfo::known_overhead_for_on_finalize()
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// AMM pool implementation
		type AMMPool: AMM<Self::AccountId, AssetId, AssetPair, Balance>;

		/// Intention resolver
		type Resolver: Resolver<Self::AccountId, Intention<Self>, Error<Self>>;

		/// Currency for transfers
		type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = AssetId, Balance = Balance, Amount = Amount>
			+ MultiReservableCurrency<Self::AccountId>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Intention registered event
		/// who, asset a, asset b, amount, intention type, intention id
		IntentionRegistered(T::AccountId, AssetId, AssetId, Balance, IntentionType, IntentionId<T>),

		/// Intention resolved as AMM Trade
		/// who, intention type, intention id, amount, amount sold/bought
		IntentionResolvedAMMTrade(T::AccountId, IntentionType, IntentionId<T>, Balance, Balance),

		/// Intention resolved as Direct Trade
		/// who, who - account between which direct trade happens
		/// intention id, intention id - intentions which are being resolved ( fully or partially )
		/// Balance, Balance  - corresponding amounts
		IntentionResolvedDirectTrade(
			T::AccountId,
			T::AccountId,
			IntentionId<T>,
			IntentionId<T>,
			Balance,
			Balance,
		),

		/// Paid fees event
		/// who, account paid to, asset, amount
		IntentionResolvedDirectTradeFees(T::AccountId, T::AccountId, AssetId, Balance),

		/// Error event - insuficient balance of specified asset
		/// who, asset, intention type, intention id, error detail
		InsufficientAssetBalanceEvent(
			T::AccountId,
			AssetId,
			IntentionType,
			IntentionId<T>,
			dispatch::DispatchError,
		),

		/// Intetion Error Event
		/// who, assets, sell or buy, intention id, error detail
		IntentionResolveErrorEvent(
			T::AccountId,
			AssetPair,
			IntentionType,
			IntentionId<T>,
			dispatch::DispatchError,
		),
	}

	#[pallet::error]
	pub enum Error<T> {
		///Token pool does not exist.
		TokenPoolNotFound,

		/// Insufficient balance
		InsufficientAssetBalance,

		/// Limit exceeded
		AssetBalanceLimitExceeded,

		/// Invalid amount
		ZeroSpotPrice,

		/// Minimum trading limit is not enough
		MinimumTradeLimitNotReached,
	}

	/// Intention count for current block
	#[pallet::storage]
	#[pallet::getter(fn get_intentions_count)]
	pub type ExchangeAssetsIntentionCount<T: Config> =
		StorageMap<_, Blake2_128Concat, (AssetId, AssetId), u32, ValueQuery>;

	/// Registered intentions for current block
	/// Stored as ( asset_a, asset_b ) combination where asset_a is meant to be exchanged for asset_b ( asset_a < asset_b)
	#[pallet::storage]
	#[pallet::getter(fn get_intentions)]
	pub type ExchangeAssetsIntentions<T: Config> =
		StorageMap<_, Blake2_128Concat, (AssetId, AssetId), Vec<Intention<T>>, ValueQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create sell intention
		/// Calculate current spot price, create an intention and store in ```ExchangeAssetsIntentions```
		#[pallet::weight(< T as Config >::WeightInfo::sell_intention() + < T as Config >::WeightInfo::on_finalize_for_one_sell_extrinsic() - < T as Config >::WeightInfo::known_overhead_for_on_finalize())]
		pub fn sell(
			origin: OriginFor<T>,
			asset_sell: AssetId,
			asset_buy: AssetId,
			amount_sell: Balance,
			min_bought: Balance,
			discount: bool,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			ensure! {
				amount_sell >= MIN_TRADING_LIMIT,
				Error::<T>::MinimumTradeLimitNotReached
			};

			let assets = AssetPair {
				asset_in: asset_sell,
				asset_out: asset_buy,
			};

			ensure!(T::AMMPool::exists(assets), Error::<T>::TokenPoolNotFound);

			ensure!(
				T::Currency::free_balance(asset_sell, &who) >= amount_sell,
				Error::<T>::InsufficientAssetBalance
			);

			let amount_buy = T::AMMPool::get_spot_price_unchecked(asset_sell, asset_buy, amount_sell);

			ensure!(amount_buy != 0, Error::<T>::ZeroSpotPrice);

			Self::register_intention(
				&who,
				IntentionType::SELL,
				assets,
				amount_sell,
				amount_buy,
				min_bought,
				discount,
			)?;

			Ok(().into())
		}

		/// Create buy intention
		/// Calculate current spot price, create an intention and store in ```ExchangeAssetsIntentions```
		#[pallet::weight(<T as Config>::WeightInfo::buy_intention() + <T as Config>::WeightInfo::on_finalize_for_one_buy_extrinsic() -  <T as Config>::WeightInfo::known_overhead_for_on_finalize())]
		pub fn buy(
			origin: OriginFor<T>,
			asset_buy: AssetId,
			asset_sell: AssetId,
			amount_buy: Balance,
			max_sold: Balance,
			discount: bool,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			ensure! {
				amount_buy >= MIN_TRADING_LIMIT,
				Error::<T>::MinimumTradeLimitNotReached
			};

			let assets = AssetPair {
				asset_in: asset_sell,
				asset_out: asset_buy,
			};

			ensure!(T::AMMPool::exists(assets), Error::<T>::TokenPoolNotFound);

			let amount_sell = T::AMMPool::get_spot_price_unchecked(asset_buy, asset_sell, amount_buy);

			ensure!(amount_sell != 0, Error::<T>::ZeroSpotPrice);

			ensure!(
				T::Currency::free_balance(asset_sell, &who) >= amount_sell,
				Error::<T>::InsufficientAssetBalance
			);

			Self::register_intention(
				&who,
				IntentionType::BUY,
				assets,
				amount_sell,
				amount_buy,
				max_sold,
				discount,
			)?;

			Ok(().into())
		}
	}
}

// "Internal" functions, callable by code.
impl<T: Config> Pallet<T> {
	/// Register SELL or BUY intention
	fn register_intention(
		who: &T::AccountId,
		intention_type: IntentionType,
		assets: AssetPair,
		amount_in: Balance,
		amount_out: Balance,
		limit: Balance,
		discount: bool,
	) -> dispatch::DispatchResult {
		let intention_count = ExchangeAssetsIntentionCount::<T>::get(assets.ordered_pair());

		let intention_id = Self::generate_intention_id(who, intention_count, &assets);

		let intention = Intention::<T> {
			who: who.clone(),
			assets,
			amount_in,
			amount_out,
			discount,
			sell_or_buy: intention_type,
			intention_id,
			trade_limit: limit,
		};
		// Note: cannot use ordered tuple pair, as this must be stored as (in,out) pair
		<ExchangeAssetsIntentions<T>>::append((assets.asset_in, assets.asset_out), intention);

		ExchangeAssetsIntentionCount::<T>::mutate(assets.ordered_pair(), |total| *total += 1u32);

		match intention_type {
			IntentionType::SELL => {
				Self::deposit_event(Event::IntentionRegistered(
					who.clone(),
					assets.asset_in,
					assets.asset_out,
					amount_in,
					intention_type,
					intention_id,
				));
			}
			IntentionType::BUY => {
				Self::deposit_event(Event::IntentionRegistered(
					who.clone(),
					assets.asset_out,
					assets.asset_in,
					amount_out,
					intention_type,
					intention_id,
				));
			}
		}

		Ok(())
	}

	/// Process intentions and attempt to match them so they can be direct traded.
	/// ```a_in_intentions``` are considered 'main' intentions.
	///
	/// This algorithm is quite simple at the moment and it tries to match as many intentions from ```b_in_intentions``` as possible while
	/// satisfying  that sum( b_in_intentions.amount_sell ) <= a_in_intention.amount_sell
	///
	/// Intention A must be valid - that means that it is verified first by validating if it was possible to do AMM trade.
	fn process_exchange_intentions(
		pair_account: &T::AccountId,
		a_in_intentions: &[Intention<T>],
		b_in_intentions: &[Intention<T>],
	) {
		let mut b_copy = b_in_intentions.to_owned();
		let mut a_copy = a_in_intentions.to_owned();

		b_copy.sort_by(|a, b| b.amount_in.cmp(&a.amount_in));
		a_copy.sort_by(|a, b| b.amount_in.cmp(&a.amount_in));

		b_copy.reverse();

		for intention in a_copy {
			if !Self::verify_intention(&intention) {
				continue;
			}

			let mut bvec = Vec::<Intention<T>>::new();
			let mut total = 0;

			while let Some(matched) = b_copy.pop() {
				bvec.push(matched.clone());
				total += matched.amount_in;

				if total >= intention.amount_in {
					break;
				}
			}

			T::Resolver::resolve_matched_intentions(pair_account, &intention, &bvec);
		}

		// If something left in b_in_intentions, just run it through AMM.
		while let Some(b_intention) = b_copy.pop() {
			T::Resolver::resolve_single_intention(&b_intention);
		}
	}

	/// Execute AMM trade.
	///
	/// Perform AMM trade with given transfer details.
	fn execute_amm_transfer(
		amm_tranfer_type: IntentionType,
		intention_id: IntentionId<T>,
		transfer: &AMMTransfer<T::AccountId, AssetPair, Balance>,
	) -> dispatch::DispatchResult {
		match amm_tranfer_type {
			IntentionType::SELL => {
				T::AMMPool::execute_sell(transfer)?;

				Self::deposit_event(Event::IntentionResolvedAMMTrade(
					transfer.origin.clone(),
					IntentionType::SELL,
					intention_id,
					transfer.amount,
					transfer.amount_out,
				));
			}
			IntentionType::BUY => {
				T::AMMPool::execute_buy(transfer)?;

				Self::deposit_event(Event::IntentionResolvedAMMTrade(
					transfer.origin.clone(),
					IntentionType::BUY,
					intention_id,
					transfer.amount,
					transfer.amount_out,
				));
			}
		};

		Ok(())
	}

	/// Send intention resolve error event.
	///
	/// Send event with error detail for intention that failed.
	fn send_intention_error_event(intention: &Intention<T>, error: dispatch::DispatchError) {
		Self::deposit_event(Event::IntentionResolveErrorEvent(
			intention.who.clone(),
			intention.assets,
			intention.sell_or_buy,
			intention.intention_id,
			error,
		));
	}

	/// Verify sell or buy intention.
	/// Perform AMM validate for given intention.
	fn verify_intention(intention: &Intention<T>) -> bool {
		match intention.sell_or_buy {
			IntentionType::SELL => {
				match T::AMMPool::validate_sell(
					&intention.who,
					intention.assets,
					intention.amount_in,
					intention.trade_limit,
					intention.discount,
				) {
					Err(error) => {
						Self::deposit_event(Event::IntentionResolveErrorEvent(
							intention.who.clone(),
							intention.assets,
							intention.sell_or_buy,
							intention.intention_id,
							error,
						));
						false
					}
					_ => true,
				}
			}
			IntentionType::BUY => {
				match T::AMMPool::validate_buy(
					&intention.who,
					intention.assets,
					intention.amount_out,
					intention.trade_limit,
					intention.discount,
				) {
					Err(error) => {
						Self::deposit_event(Event::IntentionResolveErrorEvent(
							intention.who.clone(),
							intention.assets,
							intention.sell_or_buy,
							intention.intention_id,
							error,
						));
						false
					}
					_ => true,
				}
			}
		}
	}

	fn generate_intention_id(account: &T::AccountId, c: u32, assets: &AssetPair) -> IntentionId<T> {
		let b = <system::Module<T>>::current_block_number();
		(c, &account, b, assets.ordered_pair().0, assets.ordered_pair().1).using_encoded(T::Hashing::hash)
	}
}

impl<T: Config> Resolver<T::AccountId, Intention<T>, Error<T>> for Pallet<T> {
	/// Resolve intention via AMM pool.
	fn resolve_single_intention(intention: &Intention<T>) {
		let amm_transfer = match intention.sell_or_buy {
			IntentionType::SELL => T::AMMPool::validate_sell(
				&intention.who,
				intention.assets,
				intention.amount_in,
				intention.trade_limit,
				intention.discount,
			),
			IntentionType::BUY => T::AMMPool::validate_buy(
				&intention.who,
				intention.assets,
				intention.amount_out,
				intention.trade_limit,
				intention.discount,
			),
		};

		match amm_transfer {
			Ok(x) => match Self::execute_amm_transfer(intention.sell_or_buy, intention.intention_id, &x) {
				Ok(_) => {}
				Err(error) => {
					Self::send_intention_error_event(&intention, error);
				}
			},
			Err(error) => {
				Self::send_intention_error_event(&intention, error);
			}
		};
	}

	/// Resolve main intention and corresponding matched intentions
	///
	/// For each matched intention - work out how much can be traded directly and rest is AMM traded.
	/// If there is anything left in the main intention - it is AMM traded.
	fn resolve_matched_intentions(pair_account: &T::AccountId, intention: &Intention<T>, matched: &[Intention<T>]) {
		let mut intention_copy = intention.clone();

		for matched_intention in matched.iter() {
			let amount_a_in = intention_copy.amount_in;
			let amount_a_out = intention_copy.amount_out;
			let amount_b_in = matched_intention.amount_in;
			let amount_b_out = matched_intention.amount_out;

			// There are multiple scenarios to handle
			// !. Main intention amount left > matched intention amount
			// 2. Main intention amount left < matched intention amount
			// 3. Main intention amount left = matched intention amount

			if amount_a_in > amount_b_out {
				// Scenario 1: Matched intention can be completely direct traded
				//
				// 1. Prepare direct trade details - during preparation, direct amounts are reserved.
				// 2. Execute if ok otherwise revert ( unreserve amounts if any ) .
				// 3. Sets new amount (rest amount) and trade limit accordingly.
				let mut dt = DirectTradeData::<T> {
					intention_a: &intention_copy,
					intention_b: &matched_intention,
					amount_from_a: amount_b_out,
					amount_from_b: amount_b_in,
					transfers: Vec::<Transfer<T>>::new(),
				};

				// As we direct trading the total matched intention amount - we need to check the trade limit for the matched intention
				match matched_intention.sell_or_buy {
					IntentionType::SELL => {
						if dt.amount_from_a < matched_intention.trade_limit {
							Self::send_intention_error_event(
								&matched_intention,
								Error::<T>::AssetBalanceLimitExceeded.into(),
							);
							continue;
						}
					}
					IntentionType::BUY => {
						if dt.amount_from_a > matched_intention.trade_limit {
							Self::send_intention_error_event(
								&matched_intention,
								Error::<T>::AssetBalanceLimitExceeded.into(),
							);
							continue;
						}
					}
				};

				match dt.prepare(pair_account) {
					true => {
						dt.execute();

						intention_copy.amount_in = amount_a_in - amount_b_out;
						intention_copy.amount_out = amount_a_out - amount_b_in;

						intention_copy.trade_limit = match intention_copy.sell_or_buy {
							IntentionType::SELL => intention_copy.trade_limit.saturating_sub(amount_b_in),
							IntentionType::BUY => intention_copy.trade_limit - amount_b_in,
						};
					}
					false => {
						dt.revert();
						continue;
					}
				}
			} else if amount_a_in < amount_b_out {
				// Scenario 2: Matched intention CANNOT be completely directly traded
				//
				// 1. Work out rest amount and rest trade limits for direct trades.
				// 2. Verify if AMM transfer can be successfully performed
				// 3. Verify if direct trade can be successfully performed
				// 4. If both ok - execute
				// 5. Main intention is empty at this point - just set amount to 0.
				let rest_in_diff = amount_b_in.checked_sub(amount_a_out);
				let rest_out_diff = amount_b_out.checked_sub(amount_a_in);

				if rest_in_diff.is_none() || rest_out_diff.is_none() {
					Self::send_intention_error_event(
						&matched_intention,
						Error::<T>::AssetBalanceLimitExceeded.into(), // TODO: better error here ?!
					);
					continue;
				}

				let rest_in_amount = rest_in_diff.unwrap();
				let rest_out_amount = rest_out_diff.unwrap();

				let rest_limit = matched_intention.trade_limit.saturating_sub(amount_a_in);

				let mut dt = DirectTradeData::<T> {
					intention_a: &intention_copy,
					intention_b: &matched_intention,
					amount_from_a: amount_a_in,
					amount_from_b: amount_b_in - rest_in_amount,
					transfers: Vec::<Transfer<T>>::new(),
				};

				let amm_transfer_result = match matched_intention.sell_or_buy {
					IntentionType::SELL => T::AMMPool::validate_sell(
						&matched_intention.who,
						matched_intention.assets,
						rest_in_amount,
						rest_limit,
						matched_intention.discount,
					),
					IntentionType::BUY => T::AMMPool::validate_buy(
						&matched_intention.who,
						matched_intention.assets,
						rest_out_amount,
						rest_limit,
						matched_intention.discount,
					),
				};

				let amm_transfer = match amm_transfer_result {
					Ok(x) => x,
					Err(error) => {
						Self::send_intention_error_event(&matched_intention, error);
						continue;
					}
				};

				match matched_intention.sell_or_buy {
					IntentionType::SELL => {
						if dt.amount_from_b < matched_intention.trade_limit - amm_transfer.amount_out {
							Self::send_intention_error_event(
								&matched_intention,
								Error::<T>::AssetBalanceLimitExceeded.into(),
							);
							continue;
						}
					}
					IntentionType::BUY => {
						if dt.amount_from_b > matched_intention.trade_limit - amm_transfer.amount_out {
							Self::send_intention_error_event(
								&matched_intention,
								Error::<T>::AssetBalanceLimitExceeded.into(),
							);
							continue;
						}
					}
				};

				match dt.prepare(pair_account) {
					true => {
						match Self::execute_amm_transfer(
							matched_intention.sell_or_buy,
							matched_intention.intention_id,
							&amm_transfer,
						) {
							Ok(_) => {
								dt.execute();
								intention_copy.amount_in = 0;
							}
							Err(error) => {
								Self::send_intention_error_event(&matched_intention, error);
								dt.revert();
								continue;
							}
						}
					}
					false => {
						dt.revert();
						continue;
					}
				}
			} else {
				// Scenario 3: Exact match
				//
				// 1. Prepare direct trade
				// 2. Verify and execute
				// 3. Main intention is emtpy at this point -set amount to 0.
				let mut dt = DirectTradeData::<T> {
					intention_a: &intention_copy,
					intention_b: &matched_intention,
					amount_from_a: amount_a_in,
					amount_from_b: amount_b_in,
					transfers: Vec::<Transfer<T>>::new(),
				};

				// As we direct trading the total matched intention amount - we need to check the trade limit for the matched intention
				match intention.sell_or_buy {
					IntentionType::SELL => {
						if dt.amount_from_b < intention.trade_limit {
							Self::send_intention_error_event(&intention, Error::<T>::AssetBalanceLimitExceeded.into());
							continue;
						}
					}
					IntentionType::BUY => {
						if dt.amount_from_b > intention.trade_limit {
							Self::send_intention_error_event(&intention, Error::<T>::AssetBalanceLimitExceeded.into());
							continue;
						}
					}
				};

				match matched_intention.sell_or_buy {
					IntentionType::SELL => {
						if dt.amount_from_a < matched_intention.trade_limit {
							Self::send_intention_error_event(
								&matched_intention,
								Error::<T>::AssetBalanceLimitExceeded.into(),
							);
							continue;
						}
					}
					IntentionType::BUY => {
						if dt.amount_from_a > matched_intention.trade_limit {
							Self::send_intention_error_event(
								&matched_intention,
								Error::<T>::AssetBalanceLimitExceeded.into(),
							);
							continue;
						}
					}
				};

				match dt.prepare(pair_account) {
					true => {
						dt.execute();
						intention_copy.amount_in = 0;
					}
					false => {
						dt.revert();
						continue;
					}
				}
			}
		}

		// If there is something left, just resolve as a single intention
		if intention_copy.amount_in > 0 {
			Self::resolve_single_intention(&intention_copy);
		}
	}
}
