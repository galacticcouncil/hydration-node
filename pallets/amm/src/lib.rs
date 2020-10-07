#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, dispatch, dispatch::DispatchResult, ensure, traits::Get,
};
use frame_system::{self as system, ensure_signed};
use primitives::{fee, traits::AMM, AssetId, Balance, Price};
use sp_core::crypto::UncheckedFrom;
use sp_runtime::{
	traits::{Hash, Zero},
	DispatchError, FixedPointNumber,
};
use sp_std::{marker::PhantomData, vec, vec::Vec};

use primitives::{HighPrecisionBalance, LowPrecisionBalance};

use asset_registry;

use core::convert::TryFrom;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use primitives::traits::AMMTransfer;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;

/// The pallet's configuration trait.
pub trait Trait: frame_system::Trait + asset_registry::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type AssetPairAccountId: AssetPairAccountIdFor<AssetId, Self::AccountId>;
	type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = AssetId, Balance = Balance, Amount = i128>;

	type HDXAssetId: Get<AssetId>;
}

pub trait AssetPairAccountIdFor<AssetId: Sized, AccountId: Sized> {
	fn from_assets(asset_a: AssetId, asset_b: AssetId) -> AccountId;
}

pub struct AssetPairAccountId<T: Trait>(PhantomData<T>);

impl<T: Trait> AssetPairAccountIdFor<AssetId, T::AccountId> for AssetPairAccountId<T>
where
	T::AccountId: UncheckedFrom<T::Hash> + AsRef<[u8]>,
{
	fn from_assets(asset_a: AssetId, asset_b: AssetId) -> T::AccountId {
		let mut buf = Vec::new();
		buf.extend_from_slice(b"hydradx");
		if asset_a < asset_b {
			buf.extend_from_slice(&asset_a.to_le_bytes());
			buf.extend_from_slice(&asset_b.to_le_bytes());
		} else {
			buf.extend_from_slice(&asset_b.to_le_bytes());
			buf.extend_from_slice(&asset_a.to_le_bytes());
		}
		T::AccountId::unchecked_from(T::Hashing::hash(&buf[..]))
	}
}

impl<T: Trait> Module<T> {
	fn get_token_name(asset_a: AssetId, asset_b: AssetId) -> Vec<u8> {
		let mut buf: Vec<u8> = Vec::new();
		if asset_a < asset_b {
			buf.extend_from_slice(&asset_a.to_le_bytes());
			buf.extend_from_slice(b"HDT");
			buf.extend_from_slice(&asset_b.to_le_bytes());
		} else {
			buf.extend_from_slice(&asset_b.to_le_bytes());
			buf.extend_from_slice(b"HDT");
			buf.extend_from_slice(&asset_a.to_le_bytes());
		}
		buf
	}
}

// This pallet's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as AMM {
		ShareToken get(fn share_token): map hasher(blake2_128_concat) T::AccountId => AssetId;
		TotalLiquidity get(fn total_liquidity): map hasher(blake2_128_concat) T::AccountId => Balance;

		PoolAssets get(fn pool_assets): map hasher(blake2_128_concat) T::AccountId => (AssetId, AssetId);
	}
}

// The pallet's events
decl_event!(
	pub enum Event<T>
	where
		AccountId = <T as system::Trait>::AccountId,
		AssetId = AssetId,
		Balance = Balance,
	{
		/// AddLiquidity
		/// who, asset_a, asset_b, amount_a, amount_b
		AddLiquidity(AccountId, AssetId, AssetId, Balance, Balance),
		/// who, asset_a, asset_b, shares
		RemoveLiquidity(AccountId, AssetId, AssetId, Balance),

		/// Pool creation - who, asset a, asset b, liquidity
		CreatePool(AccountId, AssetId, AssetId, Balance),

		/// Pool destroyed - who, asset a, asset b
		PoolDestroyed(AccountId, AssetId, AssetId),

		/// Sell token - who, asset sell, asset buy, amount, sale price
		Sell(AccountId, AssetId, AssetId, Balance, Balance),

		/// Buy token - who, asset buy, asset sell, amount, buy price
		Buy(AccountId, AssetId, AssetId, Balance, Balance),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {

		CannotCreatePoolWithSameAssets,

		CannotCreatePoolWithZeroLiquidity,
		CannotCreatePoolWithZeroInitialPrice,
		CannotRemoveLiquidityWithZero,

		CannotAddZeroLiquidity,

		AssetBalanceLimitExceeded,
		InsufficientAssetBalance,
		InsufficientPoolAssetBalance,
		InsufficientHDXBalance,

		InvalidSharesDivResult,
		InvalidMintedLiquidity,

		NextAssetIdUnavailable,

		TokenPoolNotFound,
		TokenPoolAlreadyExists,

		CreatePoolAssetAmountInvalid,
		CreatePoolSharesAmountInvalid,

		AddAssetAmountInvalid,
		AddSharesAmountInvalid,
		RemoveAssetAmountInvalid,
		SellAssetAmountInvalid,
		BuyAssetAmountInvalid,
		SpotPriceInvalid,
		FeeAmountInvalid,
		CannotApplyDiscount,

		InvalidLiquidityAmount,
	}
}

// The pallet's dispatchable functions.
decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		// Initializing errors
		// this includes information about your errors in the node's metadata.
		// it is needed only if you are using errors in your pallet
		type Error = Error<T>;

		// Initializing events
		// this is needed only if you are using events in your pallet
		fn deposit_event() = default;

		#[weight = 10_000]
		pub fn create_pool(
			origin,
			asset_a: AssetId,
			asset_b: AssetId,
			amount: Balance,
			initial_price: Price
		) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				!amount.is_zero(),
				Error::<T>::CannotCreatePoolWithZeroLiquidity
			);
			ensure!(
				!initial_price.is_zero(),
				Error::<T>::CannotCreatePoolWithZeroInitialPrice
			);

			ensure!(
				asset_a != asset_b,
				Error::<T>::CannotCreatePoolWithSameAssets
			);

			ensure!(
				!Self::exists(asset_a, asset_b),
				Error::<T>::TokenPoolAlreadyExists
			);

			let asset_b_amount= initial_price.checked_mul_int(amount).ok_or(Error::<T>::CreatePoolAssetAmountInvalid)?;
			let shares_added = amount.checked_mul(asset_b_amount).ok_or(Error::<T>::CreatePoolSharesAmountInvalid)?;

			ensure!(
				T::Currency::free_balance(asset_a, &who) >= amount,
				Error::<T>::InsufficientAssetBalance
			);

			ensure!(
				T::Currency::free_balance(asset_b, &who) >= asset_b_amount,
				Error::<T>::InsufficientAssetBalance
			);

			// Create pool only if amounts dont overflow

			let pair_account = Self::get_pair_id(&asset_a, &asset_b);

			let share_token = match Self::exists(asset_a, asset_b) {
				true => Self::share_token(&pair_account),
				false => {
					let token_name = Self::get_token_name(asset_a, asset_b);

					let share_token = <asset_registry::Module<T>>::create_asset(token_name)?.into();

					<ShareToken<T>>::insert(&pair_account, &share_token);
					<PoolAssets<T>>::insert(&pair_account, (asset_a, asset_b));
					share_token
				}
			};

			T::Currency::transfer(asset_a, &who, &pair_account, amount)?;
			T::Currency::transfer(asset_b, &who, &pair_account, asset_b_amount)?;

			T::Currency::deposit(share_token, &who, shares_added)?;

			<TotalLiquidity<T>>::insert(&pair_account, shares_added);

			Self::deposit_event(RawEvent::CreatePool(who, asset_a, asset_b, shares_added));

			Ok(())
		}

		#[weight = 10_000]
		pub fn add_liquidity(
			origin,
			asset_a: AssetId,
			asset_b: AssetId,
			amount_a: Balance,
			amount_b_max_limit: Balance
		) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				Self::exists(asset_a, asset_b),
				Error::<T>::TokenPoolNotFound
			);

			ensure!(
				!amount_a.is_zero(),
				Error::<T>::CannotAddZeroLiquidity
			);

			ensure!(
				!amount_b_max_limit.is_zero(),
				Error::<T>::CannotAddZeroLiquidity
			);

			ensure!(
				T::Currency::free_balance(asset_a, &who) >= amount_a,
				Error::<T>::InsufficientAssetBalance
			);

			ensure!(
				T::Currency::free_balance(asset_b, &who) >= amount_b_max_limit,
				Error::<T>::InsufficientAssetBalance
			);

			let pair_account = Self::get_pair_id(&asset_a , &asset_b );

			let share_token = Self::share_token(&pair_account);

			let asset_a_reserve = T::Currency::free_balance(asset_a, &pair_account);
			let asset_b_reserve = T::Currency::free_balance(asset_b, &pair_account);
			let total_liquidity = Self::total_liquidity(&pair_account);

			let amount_hp: HighPrecisionBalance = HighPrecisionBalance::from(amount_a).into();
			let b_reserve_hp: HighPrecisionBalance = HighPrecisionBalance::from(asset_b_reserve).into();
			let a_reserve_hp: HighPrecisionBalance = HighPrecisionBalance::from(asset_a_reserve).into();
			let liquidity_hp: HighPrecisionBalance = HighPrecisionBalance::from(total_liquidity).into();

			let b_required_hp = amount_hp.checked_mul( b_reserve_hp).expect("Cannot overflow").checked_div( a_reserve_hp).expect("Cannot panic as reserve cannot be 0");

			let b_required_lp: Result<LowPrecisionBalance, &'static str> = LowPrecisionBalance::try_from(b_required_hp);
			ensure!(b_required_lp.is_ok(), Error::<T>::AddAssetAmountInvalid);
			let amount_b_required = b_required_lp.unwrap();

			let l_minted = amount_hp.checked_mul(liquidity_hp).expect("Cannot overflow").checked_div(a_reserve_hp).expect("Cannot panic as asset reserve cannot be 0");

			let l_minted_lp: Result<LowPrecisionBalance, &'static str> = LowPrecisionBalance::try_from(l_minted);
			ensure!(l_minted_lp.is_ok(), Error::<T>::AddAssetAmountInvalid);
			let shares = l_minted_lp.unwrap();

			ensure!(
				amount_b_required <= amount_b_max_limit,
				Error::<T>::AssetBalanceLimitExceeded
			);

			ensure!(
				shares >= amount_a,
				Error::<T>::InvalidMintedLiquidity
			);

			let liquidity_amount = total_liquidity.checked_add(shares).ok_or(Error::<T>::InvalidLiquidityAmount)?;

			let asset_b_balance = T::Currency::free_balance(asset_b, &who);

			ensure!(
				asset_b_balance >= amount_b_required,
				Error::<T>::InsufficientAssetBalance
			);

			T::Currency::transfer(asset_a, &who, &pair_account, amount_a)?;
			T::Currency::transfer(asset_b, &who, &pair_account, amount_b_required)?;

			T::Currency::deposit(share_token, &who, shares)?;

			<TotalLiquidity<T>>::insert(&pair_account, liquidity_amount);

			Self::deposit_event(RawEvent::AddLiquidity(who, asset_a, asset_b, amount_a, amount_b_required));

			Ok(())
		}

		#[weight = 10_000]
		pub fn remove_liquidity(
			origin,
			asset_a: AssetId,
			asset_b: AssetId,
			amount: Balance
		) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				!amount.is_zero(),
				Error::<T>::CannotRemoveLiquidityWithZero
			);

			ensure!(
				Self::exists(asset_a, asset_b),
				Error::<T>::TokenPoolNotFound
			);

			let pair_account = Self::get_pair_id(&asset_a , &asset_b );

			let share_token = Self::share_token(&pair_account);

			let total_shares = Self::total_liquidity(&pair_account);

			ensure!(
				total_shares >= amount,
				Error::<T>::InsufficientAssetBalance
			);

			ensure!(
				T::Currency::free_balance(share_token, &who) >= amount,
				Error::<T>::InsufficientAssetBalance
			);

			ensure!(
				!total_shares.is_zero(),
				Error::<T>::CannotRemoveLiquidityWithZero
			);

			let asset_a_reserve = T::Currency::free_balance(asset_a, &pair_account);
			let asset_b_reserve = T::Currency::free_balance(asset_b, &pair_account);

			let amount_hp: HighPrecisionBalance = HighPrecisionBalance::from(amount).into();
			let b_reserve_hp: HighPrecisionBalance = HighPrecisionBalance::from(asset_b_reserve).into();
			let a_reserve_hp: HighPrecisionBalance = HighPrecisionBalance::from(asset_a_reserve).into();
			let liquidity_hp: HighPrecisionBalance = HighPrecisionBalance::from(total_shares).into();

			let remove_amount_a_hp = amount_hp.checked_mul( a_reserve_hp).expect("Cannot overflow").checked_div(liquidity_hp).expect("Cannot panic as liquidity cannot be 0");

			let remove_amount_a_lp: Result<LowPrecisionBalance, &'static str> = LowPrecisionBalance::try_from(remove_amount_a_hp);
			ensure!(remove_amount_a_lp.is_ok(), Error::<T>::RemoveAssetAmountInvalid);
			let remove_amount_a = remove_amount_a_lp.unwrap();

			let remove_amount_b_hp = b_reserve_hp.checked_mul(amount_hp).expect("Cannot overflow").checked_div(liquidity_hp).expect("Cannot panic as liquidity cannot be 0");

			let remove_amount_b_lp: Result<LowPrecisionBalance, &'static str> = LowPrecisionBalance::try_from(remove_amount_b_hp);
			ensure!(remove_amount_b_lp.is_ok(), Error::<T>::RemoveAssetAmountInvalid);
			let remove_amount_b = remove_amount_b_lp.unwrap();

			ensure!(
				T::Currency::free_balance(asset_a, &pair_account) >= remove_amount_a,
				Error::<T>::InsufficientPoolAssetBalance
			);
			ensure!(
				T::Currency::free_balance(asset_b, &pair_account) >= remove_amount_b,
				Error::<T>::InsufficientPoolAssetBalance
			);

			// Note: this check is not really needed as we already check if amount to remove >= liquidity in pool
			let liquidity_amount = total_shares.checked_sub(amount).ok_or(Error::<T>::InvalidLiquidityAmount)?;

			T::Currency::transfer(asset_a, &pair_account, &who, remove_amount_a)?;
			T::Currency::transfer(asset_b, &pair_account, &who, remove_amount_b)?;

			T::Currency::withdraw(share_token, &who, amount)?;

			<TotalLiquidity<T>>::insert(&pair_account, liquidity_amount);

			Self::deposit_event(RawEvent::RemoveLiquidity(who.clone(), asset_a, asset_b, amount));

			let total_liquidity_left = Self::total_liquidity(&pair_account);

			if total_liquidity_left == 0 {
				<ShareToken<T>>::remove(&pair_account);
				<PoolAssets<T>>::remove(&pair_account);

				Self::deposit_event(RawEvent::PoolDestroyed(who, asset_a, asset_b));
			}

			Ok(())
		}

		#[weight = 10_000]
		pub fn sell(
			origin,
			asset_sell: AssetId,
			asset_buy: AssetId,
			amount_sell: Balance,
			max_limit: Balance,
			discount: bool,
		) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			<Self as AMM<_,_,_>>::sell(&who, asset_sell, asset_buy, amount_sell, max_limit, discount)
		}

		#[weight = 10_000]
		pub fn buy(
			origin,
			asset_buy: AssetId,
			asset_sell: AssetId,
			amount_buy: Balance,
			max_limit: Balance,
			discount: bool,
		) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			<Self as AMM<_,_,_>>::buy(&who, asset_buy, asset_sell, amount_buy, max_limit, discount)
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn get_spot_price(asset_a: AssetId, asset_b: AssetId, amount: Balance) -> Balance {
		match Self::exists(asset_a, asset_b) {
			true => Self::get_spot_price_unchecked(asset_a, asset_b, amount),
			false => 0,
		}
	}

	pub fn get_sell_price(asset_a: AssetId, asset_b: AssetId, amount: Balance) -> Balance {
		match Self::exists(asset_a, asset_b) {
			true => {
				let pair_account = Self::get_pair_id(&asset_a, &asset_b);

				let asset_a_reserve = T::Currency::free_balance(asset_a, &pair_account);
				let asset_b_reserve = T::Currency::free_balance(asset_b, &pair_account);

				match Self::calculate_sell_price(asset_a_reserve, asset_b_reserve, amount) {
					Result::Ok(v) => v,
					_ => 0,
				}
			}
			false => 0,
		}
	}

	pub fn get_buy_price(asset_a: AssetId, asset_b: AssetId, amount: Balance) -> Balance {
		match Self::exists(asset_a, asset_b) {
			true => {
				let pair_account = Self::get_pair_id(&asset_a, &asset_b);

				let asset_a_reserve = T::Currency::free_balance(asset_a, &pair_account);
				let asset_b_reserve = T::Currency::free_balance(asset_b, &pair_account);

				match Self::calculate_buy_price(asset_b_reserve, asset_a_reserve, amount) {
					Result::Ok(v) => v,
					_ => 0,
				}
			}
			false => 0,
		}
	}

	pub fn get_pool_balances(pool_address: T::AccountId) -> Option<Vec<(AssetId, Balance)>> {
		let mut balances = Vec::new();

		if let Some(assets) = Self::get_pool_assets(&pool_address) {
			for item in &assets {
				let reserve = T::Currency::free_balance(*item, &pool_address);
				balances.push((item.clone(), reserve));
			}
		}
		Some(balances)
	}

	pub fn calculate_sell_price(
		sell_reserve: Balance,
		buy_reserve: Balance,
		sell_amount: Balance,
	) -> Result<Balance, dispatch::DispatchError> {
		let sell_amount_hp: HighPrecisionBalance = HighPrecisionBalance::from(sell_amount).into();
		let buy_reserve_hp: HighPrecisionBalance = HighPrecisionBalance::from(buy_reserve).into();
		let sell_reserve_hp: HighPrecisionBalance = HighPrecisionBalance::from(sell_reserve).into();

		let numerator = buy_reserve_hp.checked_mul(sell_amount_hp).unwrap();
		let denominator = sell_reserve_hp.checked_add(sell_amount_hp).unwrap();

		let sale_price_hp = numerator.checked_div(denominator).unwrap();

		let sale_price_lp: Result<LowPrecisionBalance, &'static str> = LowPrecisionBalance::try_from(sale_price_hp);
		ensure!(sale_price_lp.is_ok(), Error::<T>::SellAssetAmountInvalid);
		let sale_price = sale_price_lp.unwrap();

		let sale_price_round_up = fee::fixed_fee(sale_price).ok_or::<Error<T>>(Error::<T>::SellAssetAmountInvalid)?;

		Ok(sale_price_round_up)
	}

	pub fn calculate_buy_price(
		sell_reserve: Balance,
		buy_reserve: Balance,
		amount: Balance,
	) -> Result<Balance, DispatchError> {
		let amount_hp: HighPrecisionBalance = HighPrecisionBalance::from(amount).into();
		let buy_reserve_hp: HighPrecisionBalance = HighPrecisionBalance::from(buy_reserve).into();
		let sell_reserve_hp: HighPrecisionBalance = HighPrecisionBalance::from(sell_reserve).into();

		let numerator = sell_reserve_hp.checked_mul(amount_hp).unwrap();
		let denominator = buy_reserve_hp.checked_sub(amount_hp).unwrap();

		let buy_price_hp = numerator.checked_div(denominator).unwrap();

		let buy_price_lp: Result<LowPrecisionBalance, &'static str> = LowPrecisionBalance::try_from(buy_price_hp);
		ensure!(buy_price_lp.is_ok(), Error::<T>::BuyAssetAmountInvalid);
		let buy_price = buy_price_lp.unwrap();

		let buy_price_round_up = buy_price
			.checked_add(Balance::from(1u128))
			.ok_or::<Error<T>>(Error::<T>::BuyAssetAmountInvalid)?;

		Ok(buy_price_round_up)
	}

	fn calculate_fees(amount: Balance, discount: bool, hdx_fee: &mut Balance) -> Result<Balance, DispatchError> {
		match discount {
			true => {
				let transfer_fee = fee::get_discounted_fee(amount).ok_or::<Error<T>>(Error::<T>::FeeAmountInvalid)?;
				*hdx_fee = transfer_fee;
				Ok(transfer_fee)
			}
			false => {
				*hdx_fee = 0;
				Ok(fee::get_fee(amount).ok_or::<Error<T>>(Error::<T>::FeeAmountInvalid)?)
			}
		}
	}
}

impl<T: Trait> AMM<T::AccountId, AssetId, Balance> for Module<T> {
	fn exists(asset_a: AssetId, asset_b: AssetId) -> bool {
		let pair_account = T::AssetPairAccountId::from_assets(asset_a, asset_b);
		<ShareToken<T>>::contains_key(&pair_account)
	}

	fn get_pair_id(asset_a: &AssetId, asset_b: &AssetId) -> T::AccountId {
		T::AssetPairAccountId::from_assets(*asset_a, *asset_b)
	}

	fn get_pool_assets(pool_account_id: &T::AccountId) -> Option<Vec<AssetId>> {
		match <PoolAssets<T>>::contains_key(pool_account_id) {
			true => {
				let assets = Self::pool_assets(pool_account_id);
				Some(vec![assets.0, assets.1])
			}
			false => None,
		}
	}

	fn get_spot_price_unchecked(asset_a: AssetId, asset_b: AssetId, amount: Balance) -> Balance {
		let pair_account = Self::get_pair_id(&asset_a, &asset_b);

		let asset_a_reserve = T::Currency::free_balance(asset_a, &pair_account);
		let asset_b_reserve = T::Currency::free_balance(asset_b, &pair_account);

		match Self::calculate_spot_price(asset_a_reserve, asset_b_reserve, amount) {
			Result::Ok(v) => v,
			_ => 0,
		}
	}

	fn calculate_spot_price(
		sell_reserve: Balance,
		buy_reserve: Balance,
		amount: Balance,
	) -> Result<Balance, DispatchError> {
		let amount_hp: HighPrecisionBalance = HighPrecisionBalance::from(amount).into();
		let buy_reserve_hp: HighPrecisionBalance = HighPrecisionBalance::from(buy_reserve).into();
		let sell_reserve_hp: HighPrecisionBalance = HighPrecisionBalance::from(sell_reserve).into();

		let spot_price_hp = buy_reserve_hp
			.checked_mul(amount_hp)
			.expect("Cannot overflow")
			.checked_div(sell_reserve_hp)
			.unwrap();

		let spot_price_lp: Result<LowPrecisionBalance, &'static str> = LowPrecisionBalance::try_from(spot_price_hp);
		ensure!(spot_price_lp.is_ok(), Error::<T>::SpotPriceInvalid);

		Ok(spot_price_lp.unwrap())
	}

	fn validate_sell(
		who: &T::AccountId,
		asset_sell: AssetId,
		asset_buy: AssetId,
		amount_sell: Balance,
		min_bought: Balance,
		discount: bool,
	) -> Result<AMMTransfer<T::AccountId, AssetId, Balance>, sp_runtime::DispatchError> {
		ensure!(
			T::Currency::free_balance(asset_sell, who) >= amount_sell,
			Error::<T>::InsufficientAssetBalance
		);

		ensure!(Self::exists(asset_sell, asset_buy), Error::<T>::TokenPoolNotFound);

		// If discount, pool for Sell asset and HDX must exist
		if discount {
			ensure!(
				Self::exists(asset_sell, T::HDXAssetId::get()),
				Error::<T>::CannotApplyDiscount
			);
		}

		let pair_account = Self::get_pair_id(&asset_sell, &asset_buy);

		let asset_sell_total = T::Currency::free_balance(asset_sell, &pair_account);
		let asset_buy_total = T::Currency::free_balance(asset_buy, &pair_account);

		let mut hdx_amount = 0;

		let transfer_fee = Self::calculate_fees(amount_sell, discount, &mut hdx_amount)?;

		let sale_price = Self::calculate_sell_price(asset_sell_total, asset_buy_total, amount_sell - transfer_fee)?;

		ensure!(asset_buy_total >= sale_price, Error::<T>::InsufficientAssetBalance);

		ensure!(min_bought <= sale_price, Error::<T>::AssetBalanceLimitExceeded);

		let discount_fee = if discount && hdx_amount > 0 {
			let hdx_asset = T::HDXAssetId::get();

			let hdx_pair_account = Self::get_pair_id(&asset_sell, &hdx_asset);

			let hdx_reserve = T::Currency::free_balance(hdx_asset, &hdx_pair_account);
			let asset_reserve = T::Currency::free_balance(asset_sell, &hdx_pair_account);

			let hdx_fee_spot_price = Self::calculate_spot_price(asset_reserve, hdx_reserve, hdx_amount)?;

			ensure!(
				T::Currency::free_balance(hdx_asset, who) >= hdx_fee_spot_price,
				Error::<T>::InsufficientHDXBalance
			);

			hdx_fee_spot_price
		} else {
			Balance::zero()
		};

		let transfer = AMMTransfer {
			origin: who.clone(),
			asset_sell: asset_sell,
			asset_buy: asset_buy,
			amount: amount_sell,
			amount_out: sale_price,
			discount: discount,
			discount_amount: discount_fee,
		};

		Ok(transfer)
	}

	fn execute_sell(transfer: &AMMTransfer<T::AccountId, AssetId, Balance>) -> DispatchResult {
		let pair_account = Self::get_pair_id(&transfer.asset_sell, &transfer.asset_buy);

		if transfer.discount && transfer.discount_amount > 0u128 {
			let hdx_asset = T::HDXAssetId::get();
			T::Currency::withdraw(hdx_asset, &transfer.origin, transfer.discount_amount)?;
		}

		T::Currency::transfer(transfer.asset_sell, &transfer.origin, &pair_account, transfer.amount)?;
		T::Currency::transfer(transfer.asset_buy, &pair_account, &transfer.origin, transfer.amount_out)?;

		Self::deposit_event(Event::<T>::Sell(
			transfer.origin.clone(),
			transfer.asset_sell,
			transfer.asset_buy,
			transfer.amount,
			transfer.amount_out,
		));

		Ok(())
	}

	fn validate_buy(
		who: &T::AccountId,
		asset_buy: AssetId,
		asset_sell: AssetId,
		amount_buy: Balance,
		max_limit: Balance,
		discount: bool,
	) -> Result<AMMTransfer<T::AccountId, AssetId, Balance>, DispatchError> {
		ensure!(Self::exists(asset_sell, asset_buy), Error::<T>::TokenPoolNotFound);

		let pair_account = Self::get_pair_id(&asset_buy, &asset_sell);

		let asset_buy_reserve = T::Currency::free_balance(asset_buy, &pair_account);
		let asset_sell_reserve = T::Currency::free_balance(asset_sell, &pair_account);

		ensure!(asset_buy_reserve > amount_buy, Error::<T>::InsufficientPoolAssetBalance);

		// If discount, pool for Sell asset and HDX must exist
		if discount {
			ensure!(
				Self::exists(asset_buy, T::HDXAssetId::get()),
				Error::<T>::CannotApplyDiscount
			);
		}

		let mut hdx_amount = 0;

		let transfer_fee = Self::calculate_fees(amount_buy, discount, &mut hdx_amount)?;

		let buy_price = Self::calculate_buy_price(asset_sell_reserve, asset_buy_reserve, amount_buy + transfer_fee)?;

		ensure!(
			T::Currency::free_balance(asset_sell, who) >= buy_price,
			Error::<T>::InsufficientAssetBalance
		);

		ensure!(max_limit >= buy_price, Error::<T>::AssetBalanceLimitExceeded);

		let discount_fee = if discount && hdx_amount > 0 {
			let hdx_asset = T::HDXAssetId::get();

			let hdx_pair_account = Self::get_pair_id(&asset_buy, &hdx_asset);

			let hdx_reserve = T::Currency::free_balance(hdx_asset, &hdx_pair_account);
			let asset_reserve = T::Currency::free_balance(asset_buy, &hdx_pair_account);

			let hdx_fee_spot_price = Self::calculate_spot_price(asset_reserve, hdx_reserve, hdx_amount)?;

			ensure!(
				T::Currency::free_balance(hdx_asset, who) >= hdx_fee_spot_price,
				Error::<T>::InsufficientHDXBalance
			);
			hdx_fee_spot_price
		} else {
			Balance::zero()
		};

		let transfer = AMMTransfer {
			origin: who.clone(),
			asset_sell: asset_sell,
			asset_buy: asset_buy,
			amount: amount_buy,
			amount_out: buy_price,
			discount: discount,
			discount_amount: discount_fee,
		};

		Ok(transfer)
	}

	fn execute_buy(transfer: &AMMTransfer<T::AccountId, AssetId, Balance>) -> DispatchResult {
		let pair_account = Self::get_pair_id(&transfer.asset_sell, &transfer.asset_buy);

		if transfer.discount && transfer.discount_amount > 0 {
			let hdx_asset = T::HDXAssetId::get();
			T::Currency::withdraw(hdx_asset, &transfer.origin, transfer.discount_amount)?;
		}

		T::Currency::transfer(transfer.asset_buy, &pair_account, &transfer.origin, transfer.amount)?;
		T::Currency::transfer(
			transfer.asset_sell,
			&transfer.origin,
			&pair_account,
			transfer.amount_out,
		)?;

		Self::deposit_event(Event::<T>::Buy(
			transfer.origin.clone(),
			transfer.asset_buy,
			transfer.asset_sell,
			transfer.amount,
			transfer.amount_out,
		));

		Ok(())
	}
}
