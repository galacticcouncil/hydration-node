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

#[cfg(test)]
mod tests;
pub mod traits;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::{DispatchResult, Get};
use frame_support::RuntimeDebug;
use frame_system::{ensure_signed, pallet_prelude::OriginFor};
use orml_traits::GetByKey;
use sp_core::bounded::BoundedVec;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::Permill;

pub use pallet::*;

use weights::WeightInfo;

pub type Balance = u128;
pub type ReferralCode<S> = BoundedVec<u8, S>;

const MIN_CODE_LENGTH: usize = 3;

use scale_info::TypeInfo;

#[derive(Hash, Clone, Default, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
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

		/// Asset type
		type AssetId: frame_support::traits::tokens::AssetId + MaybeSerializeDeserialize;

		/// Support for transfers.
		type Currency: Transfer<Self::AccountId, AssetId = Self::AssetId, Balance = Balance>;

		/// Support for asset conversion.
		type Convert: Convert<Self::AccountId, Self::AssetId, Balance, Error = DispatchError>;

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

	/// Accrued amounts of an asset from trading activity.
	/// Maps an amount to asset and account. This amount needs to be converted to native currency prior to claiming as a reward.
	#[pallet::storage]
	#[pallet::getter(fn accrued)]
	pub(super) type Accrued<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::AssetId, Blake2_128Concat, T::AccountId, Balance, ValueQuery>;

	/// Accumulated rewards
	/// Reward amount of native asset per account.
	#[pallet::storage]
	#[pallet::getter(fn account_rewards)]
	pub(super) type Rewards<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Balance, ValueQuery>;

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
			amount: Balance,
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
		/// More rewards have been distributed than allowed after conversion. This is a bug.
		IncorrectRewardDistribution,
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

			let total_reward_amount =
				T::Convert::convert(Self::pot_account_id(), asset_id, T::RewardAsset::get(), asset_balance)?;

			// Keep track of amount rewarded, in case of a a leftover (due to rounding)
			let mut rewarded: Balance = 0;
			for (account, amount) in Accrued::<T>::drain_prefix(asset_id) {
				// We need to figure out how much of the reward should be assigned to the individual recipients.
				// Price = reward_asset_amount / asset_balance
				// rewarded = price * account balance
				let reward_amount = (total_reward_amount * amount) / asset_balance; // TODO: U256 and safe math please!

				Rewards::<T>::try_mutate(account.clone(), |v| -> DispatchResult {
					*v = v.checked_add(reward_amount).ok_or(ArithmeticError::Overflow)?;

					Referrer::<T>::mutate(account, |d| {
						// You might ask why do we need to have an OptionQuery here? it would be simpler to just have value query and update the account
						// However, in Rewards, not all accounts are necessarily Referrer accounts.
						// We heep there trader account which earn back some percentage of the fee. And for those, no levels!
						if let Some((level, total)) = d {
							*total = total.saturating_add(reward_amount);

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
					Ok(())
				})?;
				rewarded = rewarded.saturating_add(reward_amount);
			}

			// Should not really happy, but let's be safe and ensure that we have not distributed more than allowed.
			ensure!(rewarded <= total_reward_amount, Error::<T>::IncorrectRewardDistribution);

			// Due to rounding, there can be something left, let's just transfer it to treasury.
			let remainder = total_reward_amount.saturating_sub(rewarded);
			if remainder > 0 {
				T::Currency::transfer(
					T::RewardAsset::get(),
					&Self::pot_account_id(),
					&T::RegistrationFee::get().2,
					remainder,
					true,
				)?;
			}

			Self::deposit_event(Event::Converted {
				from: asset_id,
				to: T::RewardAsset::get(),
				amount: asset_balance,
				received: total_reward_amount,
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
			let amount = Rewards::<T>::take(&who);
			T::Currency::transfer(T::RewardAsset::get(), &Self::pot_account_id(), &who, amount, true)?;
			Self::deposit_event(Event::Claimed { who, amount });
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
}
