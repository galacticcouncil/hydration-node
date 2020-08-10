#![cfg_attr(not(feature = "std"), no_std)]

/// HydraSwap AMM Module
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, dispatch, dispatch::DispatchResult, ensure, traits::Get
};
use frame_system::{self as system, ensure_signed};
use primitives::{fee, traits::TokenPool, traits::AMM, AssetId, Balance};
use sp_core::crypto::UncheckedFrom;
use sp_runtime::{
	traits::{Hash, Zero},
	DispatchError,
};
use sp_std::{marker::PhantomData, vec::Vec};

use asset_registry;

use orml_traits::{MultiCurrency, MultiCurrencyExtended};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

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
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {
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
			initial_price: Balance
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
				!<Self as TokenPool<_,_>>::exists(asset_a, asset_b),
				Error::<T>::TokenPoolAlreadyExists
			);

			let asset_b_amount= amount.checked_mul(initial_price).ok_or(Error::<T>::CreatePoolAssetAmountInvalid)?;
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
			let (pair_account, share_token ) = <Self as TokenPool<_,_>>::create_pool(&asset_a, &asset_b)?;

			T::Currency::transfer(asset_a, &who, &pair_account, amount)?;
			T::Currency::transfer(asset_b, &who, &pair_account, asset_b_amount)?;

			T::Currency::deposit(share_token, &who, shares_added)?;

			<TotalLiquidity<T>>::mutate(&pair_account, |total| *total = total.saturating_add(shares_added));

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
				<Self as TokenPool<_,_>>::exists(asset_a, asset_b),
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

			// This should never happen if we destroy pool after removing last liquidity.
			// We will need to change how we create share token to be deterministic.
			// TODO: after we switch from generic_asset pallet

			let (shares, amount_b_required) = if Self::total_liquidity(&pair_account).is_zero() {
				(amount_a.checked_mul(amount_b_max_limit).ok_or(Error::<T>::AddAssetAmountInvalid)?, amount_b_max_limit)
			} else {
				let asset_a_total = T::Currency::free_balance(asset_a, &pair_account);
				let asset_b_total = T::Currency::free_balance(asset_b, &pair_account);
				let total_liquidity = Self::total_liquidity(&pair_account);

				let asset_b_required = amount_a
					.checked_mul(asset_b_total).ok_or(Error::<T>::AddAssetAmountInvalid)?
					.checked_div(asset_a_total).ok_or(Error::<T>::AddAssetAmountInvalid)?;

				let liquidity_minted = amount_a
					.checked_mul(total_liquidity).ok_or(Error::<T>::AddSharesAmountInvalid)?
					.checked_div(asset_a_total).ok_or(Error::<T>::AddSharesAmountInvalid)?;

				ensure!(
					asset_b_required <= amount_b_max_limit,
					Error::<T>::AssetBalanceLimitExceeded
				);

				ensure!(
					liquidity_minted >= amount_a,
					Error::<T>::InvalidMintedLiquidity
				);

				(liquidity_minted, asset_b_required)
			};

			let asset_b_balance = T::Currency::free_balance(asset_b, &who);

			ensure!(
				asset_b_balance >= amount_b_required,
				Error::<T>::InsufficientAssetBalance
			);

			T::Currency::transfer(asset_a, &who, &pair_account, amount_a)?;
			T::Currency::transfer(asset_b, &who, &pair_account, amount_b_required)?;

			T::Currency::deposit(share_token, &who, shares)?;

			<TotalLiquidity<T>>::mutate(&pair_account, |total| *total = total.saturating_add(shares));

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
				<Self as TokenPool<_,_>>::exists(asset_a, asset_b),
				Error::<T>::TokenPoolNotFound
			);

			let pair_account = Self::get_pair_id(&asset_a , &asset_b );

			let share_token = Self::share_token(&pair_account);

			let total_shares = Self::total_liquidity(&pair_account);

			ensure!(
				T::Currency::free_balance(share_token, &who) >= amount,
				Error::<T>::InsufficientAssetBalance
			);

			ensure!(
				!total_shares.is_zero(),
				Error::<T>::CannotRemoveLiquidityWithZero
			);

			let amount_a = T::Currency::free_balance(asset_a, &pair_account);
			let amount_b = T::Currency::free_balance(asset_b, &pair_account);

			let remove_amount_a =
				(amount_a.checked_mul(amount).ok_or(Error::<T>::RemoveAssetAmountInvalid)?)
					.checked_div(total_shares).ok_or(Error::<T>::RemoveAssetAmountInvalid)?;

			let remove_amount_b =
				(amount_b.checked_mul(amount).ok_or(Error::<T>::RemoveAssetAmountInvalid)?)
					.checked_div(total_shares).ok_or(Error::<T>::RemoveAssetAmountInvalid)?;

			ensure!(
				T::Currency::free_balance(asset_a, &pair_account) >= remove_amount_a,
				Error::<T>::InsufficientPoolAssetBalance
			);
			ensure!(
				T::Currency::free_balance(asset_b, &pair_account) >= remove_amount_b,
				Error::<T>::InsufficientPoolAssetBalance
			);

			T::Currency::transfer(asset_a, &pair_account, &who, remove_amount_a)?;
			T::Currency::transfer(asset_b, &pair_account, &who, remove_amount_b)?;

			T::Currency::withdraw(share_token, &who, amount)?;

			<TotalLiquidity<T>>::mutate(&pair_account, |total| *total = total.saturating_sub(amount));

			Self::deposit_event(RawEvent::RemoveLiquidity(who, asset_a, asset_b, amount));

			Ok(())
		}

		#[weight = 10_000]
		pub fn sell(
			origin,
			asset_sell: AssetId,
			asset_buy: AssetId,
			amount_sell: Balance,
			discount: bool
		) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			<Self as AMM<_,_,_>>::sell(&who, asset_sell, asset_buy, amount_sell, discount)
		}

		#[weight = 10_000]
		pub fn buy(
			origin,
			asset_buy: AssetId,
			asset_sell: AssetId,
			amount_buy: Balance,
			discount: bool
		) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			<Self as AMM<_,_,_>>::buy(&who, asset_buy, asset_sell, amount_buy, discount)
		}
	}
}

impl<T: Trait> TokenPool<T::AccountId, AssetId> for Module<T> {
	fn exists(asset_a: AssetId, asset_b: AssetId) -> bool {
		let pair_account = T::AssetPairAccountId::from_assets(asset_a, asset_b);
		<ShareToken<T>>::contains_key(&pair_account)
	}

	fn get_pair_id(asset_a: &AssetId, asset_b: &AssetId) -> T::AccountId {
		T::AssetPairAccountId::from_assets(*asset_a, *asset_b)
	}

	fn get_pool_assets(pool_account_id: &T::AccountId) -> Option<(AssetId, AssetId)> {
		match <PoolAssets<T>>::contains_key(pool_account_id) {
			true => Some(Self::pool_assets(pool_account_id)),
			false => None,
		}
	}

	/// Creates new pool, returns pair account id
	/// It is no-op if pool already exists
	fn create_pool(asset_a: &AssetId, asset_b: &AssetId) -> Result<(T::AccountId, AssetId), DispatchError> {
		let pair_account = Self::get_pair_id(asset_a, asset_b);

		let token = match Self::exists(*asset_a, *asset_b) {
			true => Self::share_token(&pair_account),
			false => {
				let token_name = Self::get_token_name(*asset_a, *asset_b);
				let share_token = <asset_registry::Module<T>>::create_asset(token_name)?.into();
				<ShareToken<T>>::insert(&pair_account, &share_token);
				<PoolAssets<T>>::insert(&pair_account, (asset_a, asset_b));
				share_token
			}
		};

		Ok((pair_account, token))
	}
}

impl<T: Trait> AMM<T::AccountId, AssetId, Balance> for Module<T> {
	fn sell(
		who: &T::AccountId,
		asset_sell: AssetId,
		asset_buy: AssetId,
		amount_sell: Balance,
		discount: bool,
	) -> DispatchResult {
		ensure!(
			T::Currency::free_balance(asset_sell, who) >= amount_sell,
			Error::<T>::InsufficientAssetBalance
		);

		ensure!(
			<Self as TokenPool<_, _>>::exists(asset_sell, asset_buy),
			Error::<T>::TokenPoolNotFound
		);

		// If discount, pool for Sell asset and HDX must exist
		if discount {
			ensure!(
				<Self as TokenPool<_, _>>::exists(asset_sell, T::HDXAssetId::get()),
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

		if discount && hdx_amount > 0 {
			let hdx_asset = T::HDXAssetId::get();

			let hdx_pair_account = Self::get_pair_id(&asset_sell, &hdx_asset);

			let hdx_reserve = T::Currency::free_balance(hdx_asset, &hdx_pair_account);
			let asset_reserve = T::Currency::free_balance(asset_sell, &hdx_pair_account);

			let hdx_fee_spot_price = Self::calculate_spot_price(asset_reserve, hdx_reserve, hdx_amount)?;

			ensure!(
				T::Currency::free_balance(hdx_asset, who) >= hdx_fee_spot_price,
				Error::<T>::InsufficientHDXBalance
			);

			T::Currency::withdraw(hdx_asset, who, hdx_fee_spot_price)?;
		}

		T::Currency::transfer(asset_sell, who, &pair_account, amount_sell)?;
		T::Currency::transfer(asset_buy, &pair_account, who, sale_price)?;

		Ok(())
	}

	fn buy(
		who: &T::AccountId,
		asset_buy: AssetId,
		asset_sell: AssetId,
		amount_buy: Balance,
		discount: bool,
	) -> DispatchResult {
		ensure!(
			<Self as TokenPool<_, _>>::exists(asset_sell, asset_buy),
			Error::<T>::TokenPoolNotFound
		);

		let pair_account = Self::get_pair_id(&asset_buy, &asset_sell);

		let asset_buy_reserve = T::Currency::free_balance(asset_buy, &pair_account);
		let asset_sell_reserve = T::Currency::free_balance(asset_sell, &pair_account);

		ensure!(asset_buy_reserve > amount_buy, Error::<T>::InsufficientPoolAssetBalance);

		// If discount, pool for Sell asset and HDX must exist
		if discount {
			ensure!(
				<Self as TokenPool<_, _>>::exists(asset_buy, T::HDXAssetId::get()),
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

		if discount && hdx_amount > 0 {
			let hdx_asset = T::HDXAssetId::get();

			let hdx_pair_account = Self::get_pair_id(&asset_buy, &hdx_asset);

			let hdx_reserve = T::Currency::free_balance(hdx_asset, &hdx_pair_account);
			let asset_reserve = T::Currency::free_balance(asset_buy, &hdx_pair_account);

			let hdx_fee_spot_price = Self::calculate_spot_price(asset_reserve, hdx_reserve, hdx_amount)?;

			ensure!(
				T::Currency::free_balance(hdx_asset, who) >= hdx_fee_spot_price,
				Error::<T>::InsufficientHDXBalance
			);

			T::Currency::withdraw(hdx_asset, who, hdx_fee_spot_price)?;
		}

		T::Currency::transfer(asset_buy, &pair_account, who, amount_buy)?;
		T::Currency::transfer(asset_sell, who, &pair_account, buy_price)?;

		Ok(())
	}

	fn calculate_sell_price(
		sell_reserve: Balance,
		buy_reserve: Balance,
		sell_amount: Balance,
	) -> Result<Balance, dispatch::DispatchError> {
		let numerator = buy_reserve
			.checked_mul(sell_amount)
			.ok_or::<Error<T>>(Error::<T>::SellAssetAmountInvalid)?;
		let denominator = sell_reserve
			.checked_add(sell_amount)
			.ok_or::<Error<T>>(Error::<T>::SellAssetAmountInvalid)?;
		let sale_price = numerator
			.checked_div(denominator)
			.ok_or::<Error<T>>(Error::<T>::SellAssetAmountInvalid)?;

		let sale_price_round_up = fee::fixed_fee(sale_price).ok_or::<Error<T>>(Error::<T>::SellAssetAmountInvalid)?;

		Ok(sale_price_round_up)
	}

	fn calculate_buy_price(sell_reserve: u128, buy_reserve: u128, amount: u128) -> Result<u128, DispatchError> {
		let numerator = sell_reserve
			.checked_mul(amount)
			.ok_or::<Error<T>>(Error::<T>::BuyAssetAmountInvalid)?;
		let denominator = buy_reserve
			.checked_sub(amount)
			.ok_or::<Error<T>>(Error::<T>::BuyAssetAmountInvalid)?;

		let buy_price = numerator
			.checked_div(denominator)
			.ok_or::<Error<T>>(Error::<T>::BuyAssetAmountInvalid)?;

		let buy_price_round_up = buy_price
			.checked_add(Balance::from(1u128))
			.ok_or::<Error<T>>(Error::<T>::BuyAssetAmountInvalid)?;

		Ok(buy_price_round_up)
	}

	fn calculate_spot_price(
		sell_reserve: Balance,
		buy_reserve: Balance,
		sell_amount: Balance,
	) -> Result<Balance, DispatchError> {
		let spot_price = buy_reserve
			.checked_mul(sell_amount)
			.ok_or::<Error<T>>(Error::<T>::SpotPriceInvalid)?
			.checked_div(sell_reserve)
			.ok_or::<Error<T>>(Error::<T>::SpotPriceInvalid)?;

		Ok(spot_price)
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
