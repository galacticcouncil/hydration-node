#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::sp_runtime::{
	traits::{Hash, Zero},
	DispatchError, FixedPointNumber,
};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, dispatch, dispatch::DispatchResult, ensure, traits::Get,
};
use frame_system::{self as system, ensure_signed};
use primitives::{asset::AssetPair, fee, traits::AMM, AssetId, Balance, Price, MAX_IN_RATIO, MAX_OUT_RATIO};
use sp_std::{marker::PhantomData, vec, vec::Vec};

use frame_support::sp_runtime::app_crypto::sp_core::crypto::UncheckedFrom;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use primitives::fee::WithFee;
use primitives::traits::AMMTransfer;
use primitives::Amount;

use orml_utilities::with_transaction_result;

#[cfg(test)]
mod mock;

mod benchmarking;
#[cfg(test)]
mod tests;

pub mod weights;

use weights::WeightInfo;

/// The pallet's configuration trait.
pub trait Config: frame_system::Config + pallet_asset_registry::Config {
	type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;

	/// Share token support
	type AssetPairAccountId: AssetPairAccountIdFor<AssetId, Self::AccountId>;

	/// Multi currency for transfer of currencies
	type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = AssetId, Balance = Balance, Amount = Amount>;

	/// Native Asset Id
	type HDXAssetId: Get<AssetId>;

	/// Weight information for the extrinsics.
	type WeightInfo: WeightInfo;

	/// Trading fee rate
	type GetExchangeFee: Get<fee::Fee>;
}

pub trait AssetPairAccountIdFor<AssetId: Sized, AccountId: Sized> {
	fn from_assets(asset_a: AssetId, asset_b: AssetId) -> AccountId;
}

pub struct AssetPairAccountId<T: Config>(PhantomData<T>);

impl<T: Config> AssetPairAccountIdFor<AssetId, T::AccountId> for AssetPairAccountId<T>
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

impl<T: Config> Module<T> {
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
	trait Store for Module<T: Config> as AMM {
		/// Asset id storage for each shared token
		ShareToken get(fn share_token): map hasher(blake2_128_concat) T::AccountId => AssetId;

		/// Total liquidity for shared token
		TotalLiquidity get(fn total_liquidity): map hasher(blake2_128_concat) T::AccountId => Balance;

		/// Assair pair for each shared token in the pool
		PoolAssets get(fn pool_assets): map hasher(blake2_128_concat) T::AccountId => (AssetId, AssetId);
	}
}

// The pallet's events
decl_event!(
	pub enum Event<T>
	where
		AccountId = <T as system::Config>::AccountId,
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

		/// Sell token - who, asset in, asset out, amount, sale price
		Sell(AccountId, AssetId, AssetId, Balance, Balance),

		/// Buy token - who, asset out, asset in, amount, buy price
		Buy(AccountId, AssetId, AssetId, Balance, Balance),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Config> {

		/// Create pool errors
		CannotCreatePoolWithSameAssets,
		CannotCreatePoolWithZeroLiquidity,
		CannotCreatePoolWithZeroInitialPrice,
		CreatePoolAssetAmountInvalid,

		/// Add / Remove liquidity errors
		CannotRemoveLiquidityWithZero,
		CannotAddZeroLiquidity,
		InvalidMintedLiquidity, // No tests - but it is currently not possible this error to occur due to previous checks in the code.
		InvalidLiquidityAmount, // no tests

		/// Balance errors
		AssetBalanceLimitExceeded,
		InsufficientAssetBalance,
		InsufficientPoolAssetBalance, // No tests
		InsufficientHDXBalance, // No tests

		/// Pool existence errors
		TokenPoolNotFound,
		TokenPoolAlreadyExists,

		/// Calculation errors
		AddAssetAmountInvalid, // no tests
		RemoveAssetAmountInvalid, // no tests
		SellAssetAmountInvalid, // no tests
		BuyAssetAmountInvalid, // no tests
		FeeAmountInvalid, // no tests
		CannotApplyDiscount,

		/// Trading Limit errors
		MaxOutRatioExceeded,
		MaxInRatioExceeded,
	}
}

// The pallet's dispatchable functions.
decl_module! {
	/// The module declaration.
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		// Initializing errors
		// this includes information about your errors in the node's metadata.
		type Error = Error<T>;

		// Initializing events
		fn deposit_event() = default;

		#[weight =  <T as Config>::WeightInfo::create_pool()]
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

			let asset_pair = AssetPair{asset_in: asset_a, asset_out: asset_b};

			ensure!(
				!Self::exists(asset_pair),
				Error::<T>::TokenPoolAlreadyExists
			);

			let asset_b_amount = initial_price.checked_mul_int(amount).ok_or(Error::<T>::CreatePoolAssetAmountInvalid)?;
			let shares_added = if asset_a < asset_b { amount } else { asset_b_amount };

			ensure!(
				T::Currency::free_balance(asset_a, &who) >= amount,
				Error::<T>::InsufficientAssetBalance
			);

			ensure!(
				T::Currency::free_balance(asset_b, &who) >= asset_b_amount,
				Error::<T>::InsufficientAssetBalance
			);

			let pair_account = Self::get_pair_id(asset_pair);

			let token_name = Self::get_token_name(asset_a, asset_b);

			with_transaction_result(|| {
				let share_token = <pallet_asset_registry::Module<T>>::get_or_create_asset(token_name)?.into();

				<ShareToken<T>>::insert(&pair_account, &share_token);
				<PoolAssets<T>>::insert(&pair_account, (asset_a, asset_b));

				T::Currency::transfer(asset_a, &who, &pair_account, amount)?;
				T::Currency::transfer(asset_b, &who, &pair_account, asset_b_amount)?;

				T::Currency::deposit(share_token, &who, shares_added)?;

				<TotalLiquidity<T>>::insert(&pair_account, shares_added);

				Self::deposit_event(RawEvent::CreatePool(who, asset_a, asset_b, shares_added));

				Ok(())
			})
		}

		#[weight =  <T as Config>::WeightInfo::add_liquidity()]
		pub fn add_liquidity(
			origin,
			asset_a: AssetId,
			asset_b: AssetId,
			amount_a: Balance,
			amount_b_max_limit: Balance
		) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			let asset_pair = AssetPair{asset_in: asset_a, asset_out: asset_b};

			ensure!(
				Self::exists(asset_pair),
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

			let pair_account = Self::get_pair_id(asset_pair);

			let share_token = Self::share_token(&pair_account);

			let asset_a_reserve = T::Currency::free_balance(asset_a, &pair_account);
			let asset_b_reserve = T::Currency::free_balance(asset_b, &pair_account);
			let total_liquidity = Self::total_liquidity(&pair_account);

			let amount_b_required = hydra_dx_math::calculate_liquidity_in(asset_a_reserve,
				asset_b_reserve,
				amount_a).ok_or(Error::<T>::AddAssetAmountInvalid)?;

			let shares_added = if asset_a < asset_b { amount_a } else { amount_b_required };

			ensure!(
				amount_b_required <= amount_b_max_limit,
				Error::<T>::AssetBalanceLimitExceeded
			);

			ensure!(
				shares_added > Zero::zero(),
				Error::<T>::InvalidMintedLiquidity
			);

			let liquidity_amount = total_liquidity.checked_add(shares_added).ok_or(Error::<T>::InvalidLiquidityAmount)?;

			let asset_b_balance = T::Currency::free_balance(asset_b, &who);

			ensure!(
				asset_b_balance >= amount_b_required,
				Error::<T>::InsufficientAssetBalance
			);

			with_transaction_result(|| {

				T::Currency::transfer(asset_a, &who, &pair_account, amount_a)?;
				T::Currency::transfer(asset_b, &who, &pair_account, amount_b_required)?;

				T::Currency::deposit(share_token, &who, shares_added)?;

				<TotalLiquidity<T>>::insert(&pair_account, liquidity_amount);

				Self::deposit_event(RawEvent::AddLiquidity(who, asset_a, asset_b, amount_a, amount_b_required));

				Ok(())

			})
		}

		#[weight =  <T as Config>::WeightInfo::remove_liquidity()]
		pub fn remove_liquidity(
			origin,
			asset_a: AssetId,
			asset_b: AssetId,
			liquidity_amount: Balance
		) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			let asset_pair = AssetPair{asset_in: asset_a, asset_out: asset_b};

			ensure!(
				!liquidity_amount.is_zero(),
				Error::<T>::CannotRemoveLiquidityWithZero
			);

			ensure!(
				Self::exists(asset_pair),
				Error::<T>::TokenPoolNotFound
			);

			let pair_account = Self::get_pair_id(asset_pair);

			let share_token = Self::share_token(&pair_account);

			let total_shares = Self::total_liquidity(&pair_account);

			ensure!(
				total_shares >= liquidity_amount,
				Error::<T>::InsufficientAssetBalance
			);

			ensure!(
				T::Currency::free_balance(share_token, &who) >= liquidity_amount,
				Error::<T>::InsufficientAssetBalance
			);

			ensure!(
				!total_shares.is_zero(),
				Error::<T>::CannotRemoveLiquidityWithZero
			);

			let asset_a_reserve = T::Currency::free_balance(asset_a, &pair_account);
			let asset_b_reserve = T::Currency::free_balance(asset_b, &pair_account);

			let liquidity_out = hydra_dx_math::calculate_liquidity_out(asset_a_reserve,
				asset_b_reserve,
				liquidity_amount,
				total_shares).ok_or(Error::<T>::RemoveAssetAmountInvalid)?;

			let (remove_amount_a, remove_amount_b) = liquidity_out;

			ensure!(
				T::Currency::free_balance(asset_a, &pair_account) >= remove_amount_a,
				Error::<T>::InsufficientPoolAssetBalance
			);
			ensure!(
				T::Currency::free_balance(asset_b, &pair_account) >= remove_amount_b,
				Error::<T>::InsufficientPoolAssetBalance
			);

			let liquidity_left = total_shares.checked_sub(liquidity_amount).ok_or(Error::<T>::InvalidLiquidityAmount)?;

			with_transaction_result( || {
				T::Currency::transfer(asset_a, &pair_account, &who, remove_amount_a)?;
				T::Currency::transfer(asset_b, &pair_account, &who, remove_amount_b)?;

				T::Currency::withdraw(share_token, &who, liquidity_amount)?;

				<TotalLiquidity<T>>::insert(&pair_account, liquidity_left);

				Self::deposit_event(RawEvent::RemoveLiquidity(who.clone(), asset_a, asset_b, liquidity_amount));

				if liquidity_left == 0 {
					<ShareToken<T>>::remove(&pair_account);
					<PoolAssets<T>>::remove(&pair_account);

					Self::deposit_event(RawEvent::PoolDestroyed(who, asset_a, asset_b));
				}

				Ok(())
			})
		}

		#[weight =  <T as Config>::WeightInfo::sell()]
		pub fn sell(
			origin,
			asset_in: AssetId,
			asset_out: AssetId,
			amount: Balance,
			max_limit: Balance,
			discount: bool,
		) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			<Self as AMM<_,_,_,_>>::sell(&who, AssetPair{asset_in, asset_out}, amount, max_limit, discount)
		}

		#[weight =  <T as Config>::WeightInfo::buy()]
		pub fn buy(
			origin,
			asset_out: AssetId,
			asset_in: AssetId,
			amount: Balance,
			max_limit: Balance,
			discount: bool,
		) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			<Self as AMM<_,_,_,_>>::buy(&who, AssetPair{asset_in, asset_out}, amount, max_limit, discount)
		}
	}
}

impl<T: Config> Module<T> {
	pub fn get_pool_balances(pool_address: T::AccountId) -> Option<Vec<(AssetId, Balance)>> {
		let mut balances = Vec::new();

		if let Some(assets) = Self::get_pool_assets(&pool_address) {
			for item in &assets {
				let reserve = T::Currency::free_balance(*item, &pool_address);
				balances.push((*item, reserve));
			}
		}
		Some(balances)
	}

	fn calculate_fees(amount: Balance, discount: bool, hdx_fee: &mut Balance) -> Result<Balance, DispatchError> {
		match discount {
			true => {
				let transfer_fee = amount
					.discounted_fee()
					.ok_or::<Error<T>>(Error::<T>::FeeAmountInvalid)?;
				*hdx_fee = transfer_fee;
				Ok(transfer_fee)
			}
			false => {
				*hdx_fee = 0;
				Ok(amount
					.just_fee(T::GetExchangeFee::get())
					.ok_or::<Error<T>>(Error::<T>::FeeAmountInvalid)?)
			}
		}
	}
}

impl<T: Config> AMM<T::AccountId, AssetId, AssetPair, Balance> for Module<T> {
	fn exists(assets: AssetPair) -> bool {
		let pair_account = T::AssetPairAccountId::from_assets(assets.asset_in, assets.asset_out);
		<ShareToken<T>>::contains_key(&pair_account)
	}

	fn get_pair_id(assets: AssetPair) -> T::AccountId {
		T::AssetPairAccountId::from_assets(assets.asset_in, assets.asset_out)
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
		let pair_account = Self::get_pair_id(AssetPair {
			asset_out: asset_a,
			asset_in: asset_b,
		});

		let asset_a_reserve = T::Currency::free_balance(asset_a, &pair_account);
		let asset_b_reserve = T::Currency::free_balance(asset_b, &pair_account);

		hydra_dx_math::calculate_spot_price(asset_a_reserve, asset_b_reserve, amount)
			.or(Some(0))
			.unwrap()
	}

	fn validate_sell(
		who: &T::AccountId,
		assets: AssetPair,
		amount: Balance,
		min_bought: Balance,
		discount: bool,
	) -> Result<AMMTransfer<T::AccountId, AssetPair, Balance>, sp_runtime::DispatchError> {
		ensure!(
			T::Currency::free_balance(assets.asset_in, who) >= amount,
			Error::<T>::InsufficientAssetBalance
		);

		ensure!(Self::exists(assets), Error::<T>::TokenPoolNotFound);

		// If discount, pool for Sell asset and HDX must exist
		if discount {
			ensure!(
				Self::exists(AssetPair {
					asset_in: assets.asset_in,
					asset_out: T::HDXAssetId::get()
				}),
				Error::<T>::CannotApplyDiscount
			);
		}

		let pair_account = Self::get_pair_id(assets);

		let asset_in_total = T::Currency::free_balance(assets.asset_in, &pair_account);
		let asset_out_total = T::Currency::free_balance(assets.asset_out, &pair_account);

		ensure!(amount <= asset_in_total / MAX_IN_RATIO, Error::<T>::MaxInRatioExceeded);

		let mut hdx_amount = 0;

		let transfer_fee = Self::calculate_fees(amount, discount, &mut hdx_amount)?;

		let sale_price =
			match hydra_dx_math::calculate_sell_price(asset_in_total, asset_out_total, amount - transfer_fee) {
				Some(x) => x,
				None => {
					return Err(Error::<T>::SellAssetAmountInvalid.into());
				}
			};

		ensure!(asset_out_total >= sale_price, Error::<T>::InsufficientAssetBalance);

		ensure!(min_bought <= sale_price, Error::<T>::AssetBalanceLimitExceeded);

		let discount_fee = if discount && hdx_amount > 0 {
			let hdx_asset = T::HDXAssetId::get();

			let hdx_pair_account = Self::get_pair_id(AssetPair {
				asset_in: assets.asset_in,
				asset_out: hdx_asset,
			});

			let hdx_reserve = T::Currency::free_balance(hdx_asset, &hdx_pair_account);
			let asset_reserve = T::Currency::free_balance(assets.asset_in, &hdx_pair_account);

			let hdx_fee_spot_price = hydra_dx_math::calculate_spot_price(asset_reserve, hdx_reserve, hdx_amount)
				.ok_or(Error::<T>::CannotApplyDiscount)?;

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
			assets,
			amount,
			amount_out: sale_price,
			discount,
			discount_amount: discount_fee,
		};

		Ok(transfer)
	}

	fn execute_sell(transfer: &AMMTransfer<T::AccountId, AssetPair, Balance>) -> DispatchResult {
		let pair_account = Self::get_pair_id(transfer.assets);

		with_transaction_result(|| {
			if transfer.discount && transfer.discount_amount > 0u128 {
				let hdx_asset = T::HDXAssetId::get();
				T::Currency::withdraw(hdx_asset, &transfer.origin, transfer.discount_amount)?;
			}

			T::Currency::transfer(
				transfer.assets.asset_in,
				&transfer.origin,
				&pair_account,
				transfer.amount,
			)?;
			T::Currency::transfer(
				transfer.assets.asset_out,
				&pair_account,
				&transfer.origin,
				transfer.amount_out,
			)?;

			Self::deposit_event(Event::<T>::Sell(
				transfer.origin.clone(),
				transfer.assets.asset_in,
				transfer.assets.asset_out,
				transfer.amount,
				transfer.amount_out,
			));

			Ok(())
		})
	}

	fn validate_buy(
		who: &T::AccountId,
		assets: AssetPair,
		amount: Balance,
		max_limit: Balance,
		discount: bool,
	) -> Result<AMMTransfer<T::AccountId, AssetPair, Balance>, DispatchError> {
		ensure!(Self::exists(assets), Error::<T>::TokenPoolNotFound);

		let pair_account = Self::get_pair_id(assets);

		let asset_out_reserve = T::Currency::free_balance(assets.asset_out, &pair_account);
		let asset_in_reserve = T::Currency::free_balance(assets.asset_in, &pair_account);

		ensure!(asset_out_reserve > amount, Error::<T>::InsufficientPoolAssetBalance);

		ensure!(
			amount <= asset_out_reserve / MAX_OUT_RATIO,
			Error::<T>::MaxOutRatioExceeded
		);

		// If discount, pool for Sell asset and HDX must exist
		if discount {
			ensure!(
				Self::exists(AssetPair {
					asset_in: assets.asset_out,
					asset_out: T::HDXAssetId::get()
				}),
				Error::<T>::CannotApplyDiscount
			);
		}

		let mut hdx_amount = 0;

		let transfer_fee = Self::calculate_fees(amount, discount, &mut hdx_amount)?;

		ensure!(
			amount + transfer_fee <= asset_out_reserve,
			Error::<T>::InsufficientPoolAssetBalance
		);

		let buy_price =
			match hydra_dx_math::calculate_buy_price(asset_in_reserve, asset_out_reserve, amount + transfer_fee) {
				Some(x) => x,
				None => {
					return Err(Error::<T>::BuyAssetAmountInvalid.into());
				}
			};

		ensure!(
			T::Currency::free_balance(assets.asset_in, who) >= buy_price,
			Error::<T>::InsufficientAssetBalance
		);

		ensure!(max_limit >= buy_price, Error::<T>::AssetBalanceLimitExceeded);

		let discount_fee = if discount && hdx_amount > 0 {
			let hdx_asset = T::HDXAssetId::get();

			let hdx_pair_account = Self::get_pair_id(AssetPair {
				asset_in: assets.asset_out,
				asset_out: hdx_asset,
			});

			let hdx_reserve = T::Currency::free_balance(hdx_asset, &hdx_pair_account);
			let asset_reserve = T::Currency::free_balance(assets.asset_out, &hdx_pair_account);

			let hdx_fee_spot_price = hydra_dx_math::calculate_spot_price(asset_reserve, hdx_reserve, hdx_amount)
				.ok_or(Error::<T>::CannotApplyDiscount)?;

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
			assets,
			amount,
			amount_out: buy_price,
			discount,
			discount_amount: discount_fee,
		};

		Ok(transfer)
	}

	fn execute_buy(transfer: &AMMTransfer<T::AccountId, AssetPair, Balance>) -> DispatchResult {
		let pair_account = Self::get_pair_id(transfer.assets);

		with_transaction_result(|| {
			if transfer.discount && transfer.discount_amount > 0 {
				let hdx_asset = T::HDXAssetId::get();
				T::Currency::withdraw(hdx_asset, &transfer.origin, transfer.discount_amount)?;
			}

			T::Currency::transfer(
				transfer.assets.asset_out,
				&pair_account,
				&transfer.origin,
				transfer.amount,
			)?;
			T::Currency::transfer(
				transfer.assets.asset_in,
				&transfer.origin,
				&pair_account,
				transfer.amount_out,
			)?;

			Self::deposit_event(Event::<T>::Buy(
				transfer.origin.clone(),
				transfer.assets.asset_out,
				transfer.assets.asset_in,
				transfer.amount,
				transfer.amount_out,
			));

			Ok(())
		})
	}
}
