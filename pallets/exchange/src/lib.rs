// This file is part of Basilisk-node.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::comparison_chain)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::unnecessary_wraps)]
#![feature(drain_filter)]

use frame_support::{dispatch, ensure};
use frame_system::{self as system, ensure_signed};

use codec::Encode;
use sp_std::vec::Vec;

use direct::{DirectTradeData, Transfer};
use frame_support::weights::Weight;
use hydradx_traits::{AMMTransfer, Resolver, AMM};
use orml_traits::{MultiCurrency, MultiCurrencyExtended, MultiReservableCurrency};
use primitives::{
	asset::AssetPair, constants::chain::MIN_TRADING_LIMIT, Amount, AssetId, Balance, ExchangeIntention, IntentionType,
};

use frame_support::sp_runtime::traits::BlockNumberProvider;
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
use frame_support::pallet_prelude::*;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
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

				let mut asset_a_ins = <ExchangeAssetsIntentions<T>>::get((asset_2, asset_1));
				let mut asset_b_ins = <ExchangeAssetsIntentions<T>>::get((asset_1, asset_2));

				//TODO: we can short circuit here if nothing in asset_b_sells and just resolve asset_a sells.

				Self::process_exchange_intentions(&pair_account, &mut asset_a_ins, &mut asset_b_ins);
			}

			ExchangeAssetsIntentionCount::<T>::remove_all(None);
			ExchangeAssetsIntentions::<T>::remove_all(None);
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
		/// [who, asset a, asset b, amount, intention type, intention id]
		IntentionRegistered(T::AccountId, AssetId, AssetId, Balance, IntentionType, IntentionId<T>),

		/// Intention resolved as AMM Trade
		/// [who, intention type, intention id, amount, amount sold/bought, pool account id]
		IntentionResolvedAMMTrade(
			T::AccountId,
			IntentionType,
			IntentionId<T>,
			Balance,
			Balance,
			T::AccountId,
		),

		/// Intention resolved as Direct Trade
		/// [account A, account B, intention id A, intention id B, amount A, amount B]
		IntentionResolvedDirectTrade(
			T::AccountId,
			T::AccountId,
			IntentionId<T>,
			IntentionId<T>,
			Balance,
			Balance,
		),

		/// Paid fees event
		/// [who, intention id, fee receiver, asset id, fee amount]
		IntentionResolvedDirectTradeFees(T::AccountId, IntentionId<T>, T::AccountId, AssetId, Balance),

		/// Error event - insufficient balance of specified asset
		/// who, asset, intention type, intention id, error detail
		InsufficientAssetBalanceEvent(
			T::AccountId,
			AssetId,
			IntentionType,
			IntentionId<T>,
			dispatch::DispatchError,
		),

		/// Intention Error Event
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

		/// Insufficient asset balance.
		InsufficientAssetBalance,

		/// Given trading limit has been exceeded (buy).
		TradeAmountExceededLimit,

		/// Given trading limit has not been reached (sell).
		TradeAmountNotReachedLimit,

		/// Overflow
		ZeroSpotPrice,

		/// Trade amount is too low.
		MinimumTradeLimitNotReached,

		/// Overflow
		IntentionCountOverflow,
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

	#[allow(dead_code)]
	#[pallet::extra_constants]
	impl<T: Config> Pallet<T> {
		fn min_trading_limit() -> Balance {
			T::AMMPool::get_min_trading_limit()
		}
	}

	#[allow(dead_code)]
	#[pallet::extra_constants]
	impl<T: Config> Pallet<T> {
		fn min_pool_liquidity() -> Balance {
			T::AMMPool::get_min_pool_liquidity()
		}
	}

	#[allow(dead_code)]
	#[pallet::extra_constants]
	impl<T: Config> Pallet<T> {
		fn max_in_ratio() -> u128 {
			T::AMMPool::get_max_in_ratio()
		}
	}

	#[allow(dead_code)]
	#[pallet::extra_constants]
	impl<T: Config> Pallet<T> {
		fn max_out_ratio() -> u128 {
			T::AMMPool::get_max_out_ratio()
		}
	}

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
		) -> DispatchResult {
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

			Ok(())
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
		) -> DispatchResult {
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

			Ok(())
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
	) -> DispatchResult {
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

		ExchangeAssetsIntentionCount::<T>::try_mutate(assets.ordered_pair(), |total| -> DispatchResult {
			*total = total.checked_add(1).ok_or(Error::<T>::IntentionCountOverflow)?;
			Ok(())
		})?;

		// Note: cannot use ordered tuple pair, as this must be stored as (in,out) pair
		<ExchangeAssetsIntentions<T>>::append((assets.asset_in, assets.asset_out), intention);

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
		a_in_intentions: &mut [Intention<T>],
		b_in_intentions: &mut [Intention<T>],
	) {
		b_in_intentions.sort_by(|a, b| b.amount_in.cmp(&a.amount_in));
		a_in_intentions.sort_by(|a, b| b.amount_in.cmp(&a.amount_in));

		// indication of how many have been already matched
		let mut to_skip: usize = 0;

		for intention in a_in_intentions {
			if !Self::verify_intention(intention) {
				continue;
			}

			let mut total_left = intention.amount_in;

			let matched_intentions: Vec<&Intention<T>> = b_in_intentions
				.iter()
				.skip(to_skip)
				.take_while(|x| {
					if total_left > 0 {
						total_left = total_left.saturating_sub(x.amount_out);
						true
					} else {
						false
					}
				})
				.collect();

			// We need to remember how many we already resolved so for next A intention,
			// we skip those
			to_skip += matched_intentions.len();

			T::Resolver::resolve_matched_intentions(pair_account, intention, &matched_intentions);
		}

		// If something left in b_in_intentions, just run it through AMM.
		b_in_intentions.iter().skip(to_skip).for_each(|x| {
			T::Resolver::resolve_single_intention(x);
		});
	}

	/// Execute AMM trade.
	///
	/// Perform AMM trade with given transfer details.
	fn execute_amm_transfer(
		amm_tranfer_type: IntentionType,
		intention_id: IntentionId<T>,
		transfer: &AMMTransfer<T::AccountId, AssetId, AssetPair, Balance>,
	) -> DispatchResult {
		match amm_tranfer_type {
			IntentionType::SELL => {
				T::AMMPool::execute_sell(transfer)?;

				Self::deposit_event(Event::IntentionResolvedAMMTrade(
					transfer.origin.clone(),
					IntentionType::SELL,
					intention_id,
					transfer.amount,
					transfer.amount_out + transfer.fee.1,
					T::AMMPool::get_pair_id(transfer.assets),
				));
			}
			IntentionType::BUY => {
				T::AMMPool::execute_buy(transfer)?;

				Self::deposit_event(Event::IntentionResolvedAMMTrade(
					transfer.origin.clone(),
					IntentionType::BUY,
					intention_id,
					transfer.amount,
					transfer.amount_out + transfer.fee.1,
					T::AMMPool::get_pair_id(transfer.assets),
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
		let b = <system::Pallet<T>>::current_block_number();
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
					Self::send_intention_error_event(intention, error);
				}
			},
			Err(error) => {
				Self::send_intention_error_event(intention, error);
			}
		};
	}

	/// Resolve main intention and corresponding matched intentions
	///
	/// For each matched intention - work out how much can be traded directly and rest is AMM traded.
	/// If there is anything left in the main intention - it is AMM traded.
	fn resolve_matched_intentions(pair_account: &T::AccountId, intention: &Intention<T>, matched: &[&Intention<T>]) {
		let mut intention_copy = intention.clone();

		for matched_intention in matched.iter() {
			let amount_a_in = intention_copy.amount_in;
			let amount_a_out = intention_copy.amount_out;
			let amount_b_in = matched_intention.amount_in;
			let amount_b_out = matched_intention.amount_out;

			// There are multiple scenarios to handle
			// 1. Main intention amount left > matched intention amount
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
					intention_b: matched_intention,
					amount_from_a: amount_b_out,
					amount_from_b: amount_b_in,
					transfers: Vec::<Transfer<T>>::new(),
				};

				// As we direct trading the total matched intention amount - we need to check the trade limit for the matched intention
				match matched_intention.sell_or_buy {
					IntentionType::SELL => {
						if dt.amount_from_a < matched_intention.trade_limit {
							Self::send_intention_error_event(
								matched_intention,
								Error::<T>::TradeAmountNotReachedLimit.into(),
							);
							continue;
						}
					}
					IntentionType::BUY => {
						if dt.amount_from_b > matched_intention.trade_limit {
							Self::send_intention_error_event(
								matched_intention,
								Error::<T>::TradeAmountExceededLimit.into(),
							);
							continue;
						}
					}
				};

				match dt.prepare(pair_account) {
					true => {
						dt.execute();

						intention_copy.amount_in = amount_a_in.checked_sub(amount_b_out).unwrap(); // Conditionally checked
						intention_copy.amount_out = if let Some(value) = amount_a_out.checked_sub(amount_b_in) {
							value
						} else {
							// This cannot really happen. IF this happens, that would mean that in/out calculation are wrong.
							// It is simply because if amount of one asset of intention A is < amount of the asset of intention B,
							// that means - the second asset's amounts have to be in the same way ( intention A amount < Intention B Amount )

							// however, we can send an error event just to be sure but we can actually panic here because the math is wrong!
							panic!("In/out calculations are wrong! Intention B amount has to be less that Intention A amount!");
						};

						intention_copy.trade_limit = match intention_copy.sell_or_buy {
							IntentionType::SELL => intention_copy.trade_limit.saturating_sub(amount_b_in),
							IntentionType::BUY => intention_copy.trade_limit - amount_b_out,
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

				let rest_out_amount = amount_b_out.checked_sub(amount_a_in).unwrap(); //Note: Conditionally checked

				let rest_in_amount = if let Some(value) = amount_b_in.checked_sub(amount_a_out) {
					value
				} else {
					// This cannot really happen. IF this happens, that would mean that in/out calculation are wrong.
					// It is simply because if amount of one asset of intention A is < amount of the asset of intention B,
					// that means - the second asset's amounts have to be in the same way ( intention A amount < Intention B Amount )

					// however, we can send an error event just to be sure but we can actually panic here because the math is wrong!
					panic!("In/out calculations are wrong! Intention B amount has to be less that Intention A amount!");
				};

				let mut dt = DirectTradeData::<T> {
					intention_a: &intention_copy,
					intention_b: matched_intention,
					amount_from_a: amount_a_in,
					amount_from_b: amount_a_out,
					transfers: Vec::<Transfer<T>>::new(),
				};

				let amm_transfer_result = match matched_intention.sell_or_buy {
					IntentionType::SELL => {
						let rest_limit = matched_intention.trade_limit.saturating_sub(amount_a_in);

						T::AMMPool::validate_sell(
							&matched_intention.who,
							matched_intention.assets,
							rest_in_amount,
							rest_limit,
							matched_intention.discount,
						)
					}
					IntentionType::BUY => {
						let rest_limit = matched_intention.trade_limit.saturating_sub(amount_a_out);

						T::AMMPool::validate_buy(
							&matched_intention.who,
							matched_intention.assets,
							rest_out_amount,
							rest_limit,
							matched_intention.discount,
						)
					}
				};

				let amm_transfer = match amm_transfer_result {
					Ok(x) => x,
					Err(error) => {
						Self::send_intention_error_event(matched_intention, error);
						continue;
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
								Self::send_intention_error_event(matched_intention, error);
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
					intention_b: matched_intention,
					amount_from_a: amount_a_in,
					amount_from_b: amount_b_in,
					transfers: Vec::<Transfer<T>>::new(),
				};

				// As we direct trading the total matched intention amount - we need to check the trade limit for the matched intention
				match intention.sell_or_buy {
					IntentionType::SELL => {
						if dt.amount_from_b < intention.trade_limit {
							Self::send_intention_error_event(intention, Error::<T>::TradeAmountNotReachedLimit.into());
							continue;
						}
					}
					IntentionType::BUY => {
						if dt.amount_from_a > intention.trade_limit {
							Self::send_intention_error_event(intention, Error::<T>::TradeAmountExceededLimit.into());
							continue;
						}
					}
				};

				match matched_intention.sell_or_buy {
					IntentionType::SELL => {
						if dt.amount_from_a < matched_intention.trade_limit {
							Self::send_intention_error_event(
								matched_intention,
								Error::<T>::TradeAmountNotReachedLimit.into(),
							);
							continue;
						}
					}
					IntentionType::BUY => {
						if dt.amount_from_b > matched_intention.trade_limit {
							Self::send_intention_error_event(
								matched_intention,
								Error::<T>::TradeAmountExceededLimit.into(),
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
