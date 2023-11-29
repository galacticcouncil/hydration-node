// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Referrals pallet
//!

#![cfg_attr(not(feature = "std"), no_std)]

mod weights;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarking;
#[cfg(test)]
mod tests;
pub mod traits;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::{DispatchResult, Get};
use frame_support::sp_runtime::FixedPointNumber;
use frame_support::traits::fungibles::Transfer;
use frame_support::{ensure, transactional, RuntimeDebug};
use frame_system::{ensure_signed, pallet_prelude::OriginFor};
use hydradx_traits::pools::SpotPriceProvider;
use orml_traits::GetByKey;
use sp_core::bounded::BoundedVec;
use sp_core::U256;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::{traits::CheckedAdd, DispatchError, Permill};

#[cfg(feature = "runtime-benchmarks")]
pub use crate::traits::BenchmarkHelper;

pub use pallet::*;

use weights::WeightInfo;

pub type Balance = u128;
pub type ReferralCode<S> = BoundedVec<u8, S>;

const MIN_CODE_LENGTH: usize = 3;

use scale_info::TypeInfo;

#[derive(Hash, Clone, Copy, Default, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum Level {
	#[default]
	Novice,
	Advanced,
	Expert,
}

impl Level {
	pub fn next_level(&self) -> Self {
		match self {
			Self::Novice => Self::Advanced,
			Self::Advanced => Self::Expert,
			Self::Expert => Self::Expert,
		}
	}
}

#[derive(Clone, Default, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Tier {
	/// Percentage of the fee that goes to the referrer.
	referrer: Permill,
	/// Percentage of the fee that goes back to the trader.
	trader: Permill,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::traits::Convert;
	use frame_support::pallet_prelude::*;
	use frame_support::sp_runtime::ArithmeticError;
	use frame_support::sp_runtime::FixedU128;
	use frame_support::traits::fungibles::{Inspect, Transfer};
	use frame_support::PalletId;
	use sp_runtime::traits::Zero;

	#[pallet::pallet]
	#[pallet::generate_store(pub(crate) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Origin that can set asset tier reward percentages.
		type AuthorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Asset type
		type AssetId: frame_support::traits::tokens::AssetId + MaybeSerializeDeserialize;

		/// Support for transfers.
		type Currency: Transfer<Self::AccountId, AssetId = Self::AssetId, Balance = Balance>;

		/// Support for asset conversion.
		type Convert: Convert<Self::AccountId, Self::AssetId, Balance, Error = DispatchError>;

		/// Price provider to use for shares calculation.
		type SpotPriceProvider: SpotPriceProvider<Self::AssetId, Price = FixedU128>;

		/// ID of an asset that is used to distribute rewards in.
		#[pallet::constant]
		type RewardAsset: Get<Self::AssetId>;

		/// Pallet id. Determines account which holds accumulated rewards in various assets.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Registration fee details.
		/// (ID of an asset which fee is to be paid in, Amount, Beneficiary account)
		#[pallet::constant]
		type RegistrationFee: Get<(Self::AssetId, Balance, Self::AccountId)>;

		/// Maximum referral code length.
		#[pallet::constant]
		type CodeLength: Get<u32>;

		/// Volume needed to next tier. If None returned, it is the last tier.
		type TierVolume: GetByKey<Level, Option<Balance>>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: BenchmarkHelper<Self::AssetId, Balance>;
	}

	/// Referral codes
	/// Maps an account to a referral code.
	#[pallet::storage]
	#[pallet::getter(fn referral_account)]
	pub(super) type ReferralCodes<T: Config> =
		StorageMap<_, Blake2_128Concat, ReferralCode<T::CodeLength>, T::AccountId>;

	/// Linked accounts.
	/// Maps an account to a referral account.
	#[pallet::storage]
	#[pallet::getter(fn linked_referral_account)]
	pub(super) type LinkedAccounts<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

	/// Shares per account.
	#[pallet::storage]
	#[pallet::getter(fn account_shares)]
	pub(super) type Shares<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Balance, ValueQuery>;

	/// Total share issuance.
	#[pallet::storage]
	#[pallet::getter(fn total_shares)]
	pub(super) type TotalShares<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// Referer level and total accumulated rewards over time.
	/// Maps referrer account to (Level, Balance). Level indicates current reward tier and Balance is used to unlock next tier level.
	/// Dev note: we use OptionQuery here because this helps to easily determine that an account if referrer account.
	#[pallet::storage]
	#[pallet::getter(fn referrer_level)]
	pub(super) type Referrer<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, (Level, Balance), OptionQuery>;

	///
	///
	#[pallet::storage]
	#[pallet::getter(fn asset_tier)]
	pub(super) type AssetTier<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::AssetId, Blake2_128Concat, Level, Tier, OptionQuery>;

	/// Information about assets that are currently in the rewards pot.
	/// Used to easily determine list of assets that need to be converted.
	#[pallet::storage]
	#[pallet::getter(fn assets)]
	pub(super) type Assets<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, ()>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		CodeRegistered {
			code: ReferralCode<T::CodeLength>,
			account: T::AccountId,
		},
		CodeLinked {
			account: T::AccountId,
			code: ReferralCode<T::CodeLength>,
			referral_account: T::AccountId,
		},
		Converted {
			from: T::AssetId,
			to: T::AssetId,
			amount: Balance,
			received: Balance,
		},
		Claimed {
			who: T::AccountId,
			rewards: Balance,
		},
		TierRewardSet {
			asset_id: T::AssetId,
			level: Level,
			referrer: Permill,
			trader: Permill,
		},
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		TooLong,
		TooShort,
		InvalidCharacter,
		AlreadyExists,
		InvalidCode,
		AlreadyLinked,
		ZeroAmount,
		/// Linking an account to the same referral account is not allowed.
		LinkNotAllowed,
		/// Calculated rewards are more than the fee amount. This can happen if percentage are incorrectly set.
		IncorrectRewardCalculation,
		/// Given referrer and trader percentages exceeds 100% percent.
		IncorrectRewardPercentage,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register new referral code.
		///
		/// `origin` pays the registration fee.
		/// `code` is assigned to the given `account`.
		///
		/// Length of the `code` must be at least `MIN_CODE_LENGTH`.
		/// Maximum length is limited to `T::CodeLength`.
		/// `code` must contain only alfa-numeric characters and all characters will be converted to upper case.
		///
		/// /// Parameters:
		/// - `origin`:
		/// - `code`:
		/// - `account`:
		///
		/// Emits `CodeRegistered` event when successful.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::register_code())]
		pub fn register_code(origin: OriginFor<T>, code: Vec<u8>, account: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let code: ReferralCode<T::CodeLength> = code.try_into().map_err(|_| Error::<T>::TooLong)?;

			ensure!(code.len() >= MIN_CODE_LENGTH, Error::<T>::TooShort);

			//TODO: can we do without cloning ? or perhaps merge with normalization
			ensure!(
				code.clone()
					.into_inner()
					.iter()
					.all(|c| char::is_alphanumeric(*c as char)),
				Error::<T>::InvalidCharacter
			);

			let code = Self::normalize_code(code);

			ReferralCodes::<T>::mutate(code.clone(), |v| -> DispatchResult {
				ensure!(v.is_none(), Error::<T>::AlreadyExists);

				let (fee_asset, fee_amount, beneficiary) = T::RegistrationFee::get();
				T::Currency::transfer(fee_asset, &who, &beneficiary, fee_amount, true)?;

				*v = Some(account.clone());
				Referrer::<T>::insert(&account, (Level::default(), Balance::zero()));
				Self::deposit_event(Event::CodeRegistered { code, account });
				Ok(())
			})
		}

		/// Link a code to an account.
		///
		/// /// Parameters:
		/// - `origin`:
		/// - `code`:
		///
		/// Emits `CodeLinked` event when successful.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::link_code())]
		pub fn link_code(origin: OriginFor<T>, code: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let code: ReferralCode<T::CodeLength> = code.try_into().map_err(|_| Error::<T>::InvalidCode)?;
			let code = Self::normalize_code(code);
			let ref_account = Self::referral_account(&code).ok_or(Error::<T>::InvalidCode)?;

			LinkedAccounts::<T>::mutate(who.clone(), |v| -> DispatchResult {
				ensure!(v.is_none(), Error::<T>::AlreadyLinked);

				ensure!(who != ref_account, Error::<T>::LinkNotAllowed);

				*v = Some(ref_account.clone());
				Self::deposit_event(Event::CodeLinked {
					account: who,
					code,
					referral_account: ref_account,
				});
				Ok(())
			})?;
			Ok(())
		}

		/// Convert accrued asset amount to native currency.
		///
		/// /// Parameters:
		/// - `origin`:
		/// - `asset_id`:
		///
		/// Emits `Converted` event when successful.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::convert())]
		pub fn convert(origin: OriginFor<T>, asset_id: T::AssetId) -> DispatchResult {
			ensure_signed(origin)?;

			let asset_balance = T::Currency::balance(asset_id, &Self::pot_account_id());
			ensure!(asset_balance > 0, Error::<T>::ZeroAmount);

			let total_reward_asset =
				T::Convert::convert(Self::pot_account_id(), asset_id, T::RewardAsset::get(), asset_balance)?;

			Assets::<T>::remove(asset_id);

			Self::deposit_event(Event::Converted {
				from: asset_id,
				to: T::RewardAsset::get(),
				amount: asset_balance,
				received: total_reward_asset,
			});

			Ok(())
		}

		/// Claim accumulated rewards
		///
		/// /// Parameters:
		/// - `origin`:
		///
		/// Emits `Claimed` event when successful.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::claim_rewards())]
		pub fn claim_rewards(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			for (asset_id, _) in Assets::<T>::drain() {
				let asset_balance = T::Currency::balance(asset_id, &Self::pot_account_id());
				T::Convert::convert(Self::pot_account_id(), asset_id, T::RewardAsset::get(), asset_balance)?;
			}
			let shares = Shares::<T>::take(&who);
			if shares == Balance::zero() {
				return Ok(());
			}

			let reward_reserve = T::Currency::balance(T::RewardAsset::get(), &Self::pot_account_id());
			let share_issuance = TotalShares::<T>::get();

			let rewards = || -> Option<Balance> {
				let shares_hp = U256::from(shares);
				let reward_reserve_hp = U256::from(reward_reserve);
				let share_issuance_hp = U256::from(share_issuance);
				let r = shares_hp
					.checked_mul(reward_reserve_hp)?
					.checked_div(share_issuance_hp)?;
				Balance::try_from(r).ok()
			}()
			.ok_or(ArithmeticError::Overflow)?;

			T::Currency::transfer(T::RewardAsset::get(), &Self::pot_account_id(), &who, rewards, true)?;
			TotalShares::<T>::mutate(|v| {
				*v = v.saturating_sub(shares);
			});
			Referrer::<T>::mutate(who.clone(), |v| {
				if let Some((level, total)) = v {
					*total = total.saturating_add(rewards);

					let next_tier = T::TierVolume::get(level);
					if let Some(amount_needed) = next_tier {
						if *total >= amount_needed {
							*level = level.next_level();
							// let's check if we can skip two levels
							let next_tier = T::TierVolume::get(level);
							if let Some(amount_needed) = next_tier {
								if *total >= amount_needed {
									*level = level.next_level();
								}
							}
						}
					}
				}
			});

			Self::deposit_event(Event::Claimed { who, rewards });
			Ok(())
		}

		/// Set asset tier reward percentages
		///
		/// /// Parameters:
		/// - `origin`:
		/// - `level`:
		/// - `referrer`:
		/// - `trader`:
		///
		/// Emits `Claimed` event when successful.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::set_reward_percentage())]
		pub fn set_reward_percentage(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			level: Level,
			referrer: Permill,
			trader: Permill,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			//ensure that total percentage does not exceed 100%
			ensure!(
				referrer.checked_add(&trader).is_some(),
				Error::<T>::IncorrectRewardPercentage
			);

			AssetTier::<T>::mutate(asset_id, level, |v| {
				*v = Some(Tier { referrer, trader });
			});
			Self::deposit_event(Event::TierRewardSet {
				asset_id,
				level,
				referrer,
				trader,
			});
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	//TODO: when added to runtime, make sure the account is added to the whitelist of account that cannot be dusted
	pub fn pot_account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	pub(crate) fn normalize_code(code: ReferralCode<T::CodeLength>) -> ReferralCode<T::CodeLength> {
		let r = code.into_inner().iter().map(|v| v.to_ascii_uppercase()).collect();
		ReferralCode::<T::CodeLength>::truncate_from(r)
	}

	/// Process trader fee
	/// `source`: account to take the fee from
	/// `trader`: account that does the trade
	///
	/// Returns unused amount on success.
	#[transactional]
	pub fn process_trade_fee(
		source: T::AccountId,
		trader: T::AccountId,
		asset_id: T::AssetId,
		amount: Balance,
	) -> Result<Balance, DispatchError> {
		// Does trader have a linked referral account ?
		let Some(ref_account) = Self::linked_referral_account(&trader) else {
			return Ok(amount);
		};
		// What is the referer level?
		let Some((level,_)) = Self::referrer_level(&ref_account) else {
			// Should not really happen, the ref entry should be always there.
			return Ok(amount);
		};

		// What is asset fee for this level? if any.
		let Some(tier) = Self::asset_tier(asset_id, level) else {
			return Ok(amount);
		};

		let Some(price) = T::SpotPriceProvider::spot_price(T::RewardAsset::get(), asset_id) else {
			// no price, no fun.
			return Ok(amount);
		};

		let referrer_reward = tier.referrer.mul_floor(amount);
		let trader_reward = tier.trader.mul_floor(amount);
		let total_taken = referrer_reward.saturating_add(trader_reward);
		ensure!(total_taken <= amount, Error::<T>::IncorrectRewardCalculation);
		T::Currency::transfer(asset_id, &source, &Self::pot_account_id(), total_taken, true)?;

		let referrer_shares = price.saturating_mul_int(referrer_reward);
		let trader_shares = price.saturating_mul_int(trader_reward);
		TotalShares::<T>::mutate(|v| {
			*v = v.saturating_add(referrer_shares.saturating_add(trader_shares));
		});
		Shares::<T>::mutate(ref_account, |v| {
			*v = v.saturating_add(referrer_shares);
		});
		Shares::<T>::mutate(trader, |v| {
			*v = v.saturating_add(trader_shares);
		});
		if asset_id != T::RewardAsset::get() {
			Assets::<T>::insert(asset_id, ());
		}
		Ok(amount.saturating_sub(total_taken))
	}
}
