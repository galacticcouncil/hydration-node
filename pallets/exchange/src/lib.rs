#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::comparison_chain)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, dispatch, ensure, storage::IterableStorageMap};
use frame_system::{self as system, ensure_signed};

use sp_std::vec::Vec;

use primitives::{
	fee,
	traits::{Resolver, AMM},
	AssetId, Balance, ExchangeIntention, IntentionId, IntentionType,
};
use sp_std::borrow::ToOwned;
use sp_std::cmp;

use orml_traits::{MultiCurrency, MultiCurrencyExtended, MultiReservableCurrency};

use direct::{DirectTradeData, Transfer};
use frame_support::weights::Weight;
use primitives::traits::AMMTransfer;

#[cfg(test)]
mod mock;

mod default_weights;

mod direct;
#[cfg(test)]
mod tests;

pub trait WeightInfo {
	fn known_overhead_for_on_finalize() -> Weight;
	fn sell_intention() -> Weight;
	fn buy_intention() -> Weight;

	fn on_finalize(t: u32) -> Weight;
	fn on_finalize_buys_no_matches(t: u32) -> Weight;
	fn on_finalize_sells_no_matches(t: u32) -> Weight;
	fn sell_extrinsic() -> Weight;
	fn buy_extrinsic() -> Weight;
	fn on_finalize_for_one_sell_extrinsic() -> Weight;
	fn on_finalize_for_one_buy_extrinsic() -> Weight;
}

/// The pallet's configuration trait.
pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

	/// AMM pool implementation
	type AMMPool: AMM<Self::AccountId, AssetId, Balance>;

	/// Intention resolver
	type Resolver: Resolver<Self::AccountId, ExchangeIntention<Self::AccountId, AssetId, Balance>, Error<Self>>;

	/// Currecny for transfers
	type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = AssetId, Balance = Balance, Amount = i128>
		+ MultiReservableCurrency<Self::AccountId>;

	/// Weight information for the extrinsics.
	type WeightInfo: WeightInfo;
}

/// Intention alias
pub type Intention<T> = ExchangeIntention<<T as system::Trait>::AccountId, AssetId, Balance>;

// This pallet's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as Exchange {

		/// Current intention count for current block
		ExchangeAssetsIntentionCount get(fn get_intentions_count): map hasher(blake2_128_concat) (AssetId, AssetId) => u32;

		/// Registered intentions for current block
		/// Always stored for ( asset_a, asset_b ) combination where asset_a < asset_B
		ExchangeAssetsIntentions get(fn get_intentions): map hasher(blake2_128_concat) (AssetId, AssetId) => Vec<Intention<T>>;

		/// Intention id
		Nonce: u128;
	}
}

// The pallet's events
decl_event!(
	pub enum Event<T>
	where
		AccountId = <T as system::Trait>::AccountId,
	{
		/// Intention registered event
		/// who, asset a, asset b, amount, intention type, intention id
		IntentionRegistered(AccountId, AssetId, AssetId, Balance, IntentionType, IntentionId),

		/// Intention resolved as AMM Trade
		/// who, intention type, intention id, amount, amount sold/bought
		IntentionResolvedAMMTrade(AccountId, IntentionType, IntentionId, Balance, Balance),

		IntentionResolvedDirectTrade(AccountId, AccountId, IntentionId, IntentionId, Balance, Balance),
		IntentionResolvedDirectTradeFees(AccountId, AccountId, AssetId, Balance),

		InsufficientAssetBalanceEvent(AccountId, AssetId, IntentionType, IntentionId, dispatch::DispatchError),

		//Note: This event can be used instead of AMMSellErrorEvent, AMMBuyErrorEvent
		IntentionResolveErrorEvent(
			AccountId,
			AssetId,
			AssetId,
			IntentionType,
			IntentionId,
			dispatch::DispatchError,
		),

		AMMSellErrorEvent(
			AccountId,
			AssetId,
			AssetId,
			IntentionType,
			IntentionId,
			dispatch::DispatchError,
		),
		AMMBuyErrorEvent(
			AccountId,
			AssetId,
			AssetId,
			IntentionType,
			IntentionId,
			dispatch::DispatchError,
		),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Value was None
		NoneValue,

		/// Value reached maximum and cannot be incremented further
		StorageOverflow,

		///Token pool does not exists.
		TokenPoolNotFound,

		/// Insufficient balance
		InsufficientAssetBalance,

		/// Limit exceeded
		AssetBalanceLimitExceeded,
	}
}

decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		type Error = Error<T>;

		fn deposit_event() = default;

		/// Create sell intention
		/// Calculate current spot price, create an intention and store in ```ExchangeAssetsIntentions```
		#[weight =  <T as Trait>::WeightInfo::sell_intention() + <T as Trait>::WeightInfo::on_finalize_for_one_sell_extrinsic() -  <T as Trait>::WeightInfo::known_overhead_for_on_finalize()]
		pub fn sell(
			origin,
			asset_sell: AssetId,
			asset_buy: AssetId,
			amount_sell: Balance,
			min_bought: Balance,
			discount: bool,
		)  -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::AMMPool::exists(asset_sell, asset_buy),
				Error::<T>::TokenPoolNotFound
			);

			ensure!(
				T::Currency::free_balance(asset_sell, &who) >= amount_sell,
				Error::<T>::InsufficientAssetBalance
			);

			let amount_buy = T::AMMPool::get_spot_price_unchecked(asset_sell, asset_buy, amount_sell);

			let intention = Intention::<T> {
					who: who.clone(),
					asset_sell,
					asset_buy,
					amount_sell,
					amount_buy,
					discount,
					sell_or_buy : IntentionType::SELL,
					intention_id: Nonce::get(),
					trade_limit: min_bought
			};

			<ExchangeAssetsIntentions<T>>::append((intention.asset_sell, intention.asset_buy), intention.clone());

			let asset_1 = cmp::min(intention.asset_sell, intention.asset_buy);
			let asset_2 = cmp::max(intention.asset_sell, intention.asset_buy);

			ExchangeAssetsIntentionCount::mutate((asset_1,asset_2), |total| *total += 1u32);

			Self::deposit_event(RawEvent::IntentionRegistered(who, asset_sell, asset_buy, amount_sell, IntentionType::SELL, intention.intention_id));

			Nonce::mutate(|n| *n += 1);

			Ok(())
		}

		/// Create buy intention
		/// Calculate current spot price, create an intention and store in ```ExchangeAssetsIntentions```
		#[weight =  <T as Trait>::WeightInfo::buy_intention() + <T as Trait>::WeightInfo::on_finalize_for_one_buy_extrinsic() -  <T as Trait>::WeightInfo::known_overhead_for_on_finalize()]
		pub fn buy(
			origin,
			asset_buy: AssetId,
			asset_sell: AssetId,
			amount_buy: Balance,
			max_sold: Balance,
			discount: bool,
		)  -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::AMMPool::exists(asset_sell, asset_buy),
				Error::<T>::TokenPoolNotFound
			);

			let amount_sell = T::AMMPool::get_spot_price_unchecked(asset_buy, asset_sell, amount_buy);

			ensure!(
				T::Currency::free_balance(asset_sell, &who) >= amount_sell,
				Error::<T>::InsufficientAssetBalance
			);

			let intention = Intention::<T> {
					who: who.clone(),
					asset_sell,
					asset_buy,
					amount_sell,
					amount_buy,
					sell_or_buy: IntentionType::BUY,
					discount,
					intention_id: Nonce::get(),
					trade_limit: max_sold
			};

			<ExchangeAssetsIntentions<T>>::append((intention.asset_sell, intention.asset_buy), intention.clone());

			let asset_1 = cmp::min(intention.asset_sell, intention.asset_buy);
			let asset_2 = cmp::max(intention.asset_sell, intention.asset_buy);

			ExchangeAssetsIntentionCount::mutate((asset_1,asset_2), |total| *total += 1u32);

			Self::deposit_event(RawEvent::IntentionRegistered(who, asset_buy, asset_sell, amount_buy, IntentionType::BUY, intention.intention_id));

			Nonce::mutate(|n| *n += 1);

			Ok(())
		}

		fn on_initialize() -> Weight {
			T::WeightInfo::known_overhead_for_on_finalize()
		}

		/// Finalize and resolve all registered intentions.
		/// Group/match intentions which can be directly traded.
		fn on_finalize(){

			for ((asset_1,asset_2), count) in ExchangeAssetsIntentionCount::iter() {
				// If no intention registered for asset1/2, move onto next one
				if count == 0u32 {
					continue;
				}

				let pair_account = T::AMMPool::get_pair_id(&asset_1, &asset_2);

				let asset_a_sells = <ExchangeAssetsIntentions<T>>::get((asset_2, asset_1));
				let asset_b_sells = <ExchangeAssetsIntentions<T>>::get((asset_1, asset_2));

				//TODO: we can short circuit here if nothing in asset_b_sells and just resolve asset a sells.

				Self::process_exchange_intentions(&pair_account, &asset_a_sells, &asset_b_sells);
			}

			ExchangeAssetsIntentionCount::remove_all();
			ExchangeAssetsIntentions::<T>::remove_all();
		}
	}
}

// "Internal" functions, callable by code.
impl<T: Trait> Module<T> {
	/// Process intentions and attempt to match them so they can be direct traded.
	/// ```sell_a_intentions``` are considered 'main' intentions.
	///
	/// This algorithm is quite simple at the moment and it tries to match as many intentions from ```sell_b_intentions``` as possible while
	/// satisfying  that sum( sell_b_intentions.amount_sell ) <= sell_a_intention.amount_sell
	///
	/// Intention A must be valid - that means that it is verified first by validating if it was possible to do AMM trade.
	fn process_exchange_intentions(
		pair_account: &T::AccountId,
		sell_a_intentions: &[Intention<T>],
		sell_b_intentions: &[Intention<T>],
	) {
		let mut b_copy = sell_b_intentions.to_owned();
		let mut a_copy = sell_a_intentions.to_owned();

		b_copy.sort_by(|a, b| b.amount_sell.cmp(&a.amount_sell));
		a_copy.sort_by(|a, b| b.amount_sell.cmp(&a.amount_sell));

		for intention in a_copy {
			if !Self::verify_intention(&intention) {
				continue;
			}

			let mut bvec = Vec::<Intention<T>>::new();
			let mut total = 0;
			let mut idx: usize = 0;

			while let Some(matched) = b_copy.get(idx) {
				bvec.push(matched.clone());
				total += matched.amount_sell;
				b_copy.remove(idx);
				idx += 1;

				if total >= intention.amount_sell {
					break;
				}
			}

			T::Resolver::resolve_matched_intentions(pair_account, &intention, &bvec);
		}

		// If something left in sell_b_intentions, just run it throught AMM.
		while let Some(b_intention) = b_copy.pop() {
			T::Resolver::resolve_single_intention(&b_intention);
		}
	}

	/// Execute AMM trade.
	///
	/// This performs AMM trade with given transfer details.
	fn execute_amm_transfer(
		amm_tranfer_type: IntentionType,
		intention_id: IntentionId,
		transfer: &AMMTransfer<T::AccountId, AssetId, Balance>,
	) -> dispatch::DispatchResult {
		match amm_tranfer_type {
			IntentionType::SELL => {
				T::AMMPool::execute_sell(transfer)?;

				Self::deposit_event(RawEvent::IntentionResolvedAMMTrade(
					transfer.origin.clone(),
					IntentionType::SELL,
					intention_id,
					transfer.amount,
					transfer.amount_out,
				));
			}
			IntentionType::BUY => {
				T::AMMPool::execute_buy(transfer)?;

				Self::deposit_event(RawEvent::IntentionResolvedAMMTrade(
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
	/// Sends event with error detail for intention that failed.
	fn send_intention_error_event(intention: &Intention<T>, error: dispatch::DispatchError) {
		Self::deposit_event(RawEvent::IntentionResolveErrorEvent(
			intention.who.clone(),
			intention.asset_sell,
			intention.asset_buy,
			intention.sell_or_buy.clone(),
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
					intention.asset_sell,
					intention.asset_buy,
					intention.amount_sell,
					intention.trade_limit,
					intention.discount,
				) {
					Err(error) => {
						Self::deposit_event(RawEvent::AMMSellErrorEvent(
							intention.who.clone(),
							intention.asset_sell,
							intention.asset_buy,
							intention.sell_or_buy.clone(),
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
					intention.asset_buy,
					intention.asset_sell,
					intention.amount_buy,
					intention.trade_limit,
					intention.discount,
				) {
					Err(error) => {
						Self::deposit_event(RawEvent::AMMBuyErrorEvent(
							intention.who.clone(),
							intention.asset_buy,
							intention.asset_sell,
							intention.sell_or_buy.clone(),
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
}

impl<T: Trait> Resolver<T::AccountId, ExchangeIntention<T::AccountId, AssetId, Balance>, Error<T>> for Module<T> {
	/// Resolve intention via AMM pool.
	fn resolve_single_intention(intention: &ExchangeIntention<T::AccountId, AssetId, Balance>) {
		let amm_transfer = match intention.sell_or_buy {
			IntentionType::SELL => T::AMMPool::validate_sell(
				&intention.who,
				intention.asset_sell,
				intention.asset_buy,
				intention.amount_sell,
				intention.trade_limit,
				intention.discount,
			),
			IntentionType::BUY => T::AMMPool::validate_buy(
				&intention.who,
				intention.asset_buy,
				intention.asset_sell,
				intention.amount_buy,
				intention.trade_limit,
				intention.discount,
			),
		};

		match amm_transfer {
			Ok(x) => match Self::execute_amm_transfer(intention.sell_or_buy.clone(), intention.intention_id, &x) {
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

	/// Resolve main intention and corresponding matched intention
	///
	/// For each matched intention - it works out how much can be traded directly and rest is AMM traded.
	/// If there is anything left in the main intention - it is AMM traded.
	fn resolve_matched_intentions(
		pair_account: &T::AccountId,
		intention: &ExchangeIntention<T::AccountId, AssetId, Balance>,
		matched: &[ExchangeIntention<T::AccountId, AssetId, Balance>],
	) {
		let mut intention_copy = intention.clone();

		for matched_intention in matched.iter() {
			let amount_a_sell = intention_copy.amount_sell;
			let amount_a_buy = intention_copy.amount_buy;
			let amount_b_sell = matched_intention.amount_sell;
			let amount_b_buy = matched_intention.amount_buy;

			// There are multiple scenarios to handle
			// !. Main intention amount left > matched intention amount
			// 2. Main intention amount left < matched intention amount
			// 3. Main intention amount left = matched intention amount

			if amount_a_sell > amount_b_buy {
				// Scenario 1: Matched intention can be completely directly traded
				//
				// 1. Prepare direct trade details - during preparation, direct amounts are reserved.
				// 2. Execute if ok otherwise revert ( unreserve amounts if any ) .
				// 3. Sets new amount (rest amount) and trade limit accordingly.
				let mut dt = DirectTradeData::<T> {
					intention_a: &intention_copy,
					intention_b: &matched_intention,
					amount_from_a: amount_b_buy,
					amount_from_b: amount_b_sell,
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

						intention_copy.amount_sell = amount_a_sell - amount_b_buy;
						intention_copy.amount_buy = amount_a_buy - amount_b_sell;

						intention_copy.trade_limit = match intention_copy.sell_or_buy {
							IntentionType::SELL => intention_copy.trade_limit.saturating_sub(amount_b_sell),
							IntentionType::BUY => intention_copy.trade_limit - amount_b_sell,
						};
					}
					false => {
						dt.revert();
						continue;
					}
				}
			} else if amount_a_sell < amount_b_buy {
				// Scenario 2: Matched intention CANNOT be completely directly traded
				//
				// 1. Work out rest amount and rest trade limits for direct trades.
				// 2. Verify if AMM transfer can be successfully performed
				// 3. Verify if direct trade can be successfully performed
				// 4. If both ok - execute
				// 5. Main intention is emtpy at this point - just set amount to 0.
				let rest_sell_amount = amount_b_sell - amount_a_buy;
				let rest_buy_amount = amount_b_buy - amount_a_sell;

				let rest_limit = match matched_intention.sell_or_buy {
					IntentionType::SELL => matched_intention.trade_limit.saturating_sub(amount_a_sell),
					IntentionType::BUY => matched_intention.trade_limit - amount_a_sell,
				};

				let mut dt = DirectTradeData::<T> {
					intention_a: &intention_copy,
					intention_b: &matched_intention,
					amount_from_a: amount_a_sell,
					amount_from_b: amount_b_sell - rest_sell_amount,
					transfers: Vec::<Transfer<T>>::new(),
				};

				let amm_transfer_result = match matched_intention.sell_or_buy {
					IntentionType::SELL => T::AMMPool::validate_sell(
						&matched_intention.who,
						matched_intention.asset_sell,
						matched_intention.asset_buy,
						rest_sell_amount,
						rest_limit,
						matched_intention.discount,
					),
					IntentionType::BUY => T::AMMPool::validate_buy(
						&matched_intention.who,
						matched_intention.asset_buy,
						matched_intention.asset_sell,
						rest_buy_amount,
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
							matched_intention.sell_or_buy.clone(),
							matched_intention.intention_id,
							&amm_transfer,
						) {
							Ok(_) => {
								dt.execute();
								intention_copy.amount_sell = 0;
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
					amount_from_a: amount_a_sell,
					amount_from_b: amount_b_sell,
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
						intention_copy.amount_sell = 0;
					}
					false => {
						dt.revert();
						continue;
					}
				}
			}
		}

		// If there is something left, just resolve as single intention
		if intention_copy.amount_sell > 0 {
			Self::resolve_single_intention(&intention_copy);
		}
	}
}
