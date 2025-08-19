// This file is part of hydration-node.

// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
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

// GOAT — Generalized Ownership Assignment Transfer (Hydration-ready)
// Bounty framework with multi-asset escrow, partial payouts, and cliff vesting (internal pull-claim).
//
// Tailored for Hydration runtime stacks:
// - Uses ORML `MultiCurrency` / `MultiReservableCurrency` (same abstraction Hydration uses).
// - AssetId/Balance generic, compatible with `primitives::{AssetId, Balance}`.
// - Tracks per-(bounty, asset) remaining to prevent overpay/refund bugs.
// - Bounded storage everywhere to avoid state growth DoS.
// - Internal vesting engine with cliff and catch-up behavior; funds remain in escrow until claimed.
// - Optional: you can swap internal vesting with orml-vesting by delegating in `process_payout_slice`.

pub use pallet::*;

use scale_info::TypeInfo;
use sp_runtime::traits::{AccountIdConversion, AtLeast32BitUnsigned, Saturating, Zero};
use sp_std::{collections::btree_set::BTreeSet, vec::Vec};

use frame_support::{
	pallet_prelude::*,
	PalletId,
};
use frame_system::pallet_prelude::*;

use orml_traits::{MultiCurrency, MultiReservableCurrency};

pub mod types;
pub use types::*;

// ------------------------------------
// Pallet
// ------------------------------------

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + TypeInfo {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Balance type (Hydration uses `primitives::Balance`).
		type Balance: Parameter
		+ Member
		+ Copy
		+ PartialOrd
		+ Default
		+ AtLeast32BitUnsigned
		+ MaxEncodedLen
		+ sp_std::fmt::Debug;

		/// AssetId type (Hydration uses `primitives::AssetId`).
		type AssetId: Parameter + Member + Copy + PartialOrd + Ord + Default + MaxEncodedLen + sp_std::fmt::Debug;

		/// Multi-currency abstraction (Hydration stack uses ORML `MultiCurrency`).
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Self::Balance>
		+ MultiReservableCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Self::Balance>;

		/// Pallet identifier for escrow account.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Bounds
		#[pallet::constant]
		type MaxRewardsPerBounty: Get<u32>;
		#[pallet::constant]
		type MaxApplicationsPerBounty: Get<u32>;
		#[pallet::constant]
		type MaxPayoutsPerBounty: Get<u32>;
		#[pallet::constant]
		type MaxSchedulesPerBounty: Get<u32>;
		#[pallet::constant]
		type MaxMetadataLen: Get<u32>;

		/// Weights
		type WeightInfo: WeightInfo;
	}

	// ---- Aliases / bounded collections

	type BoundedMetadataOf<T> = BoundedVec<u8, <T as Config>::MaxMetadataLen>;

	type RewardsOf<T> = BoundedVec<
		Reward<
			<T as Config>::AssetId,
			<T as Config>::Balance,
			BlockNumberFor<T>,
		>,
		<T as Config>::MaxRewardsPerBounty,
	>;

	type ApplicationsOf<T> = BoundedVec<
		Application<
			<T as frame_system::Config>::AccountId,
			BlockNumberFor<T>,
			BoundedMetadataOf<T>,
		>,
		<T as Config>::MaxApplicationsPerBounty,
	>;

	type PayoutsOf<T> = BoundedVec<
		Payout<
			<T as frame_system::Config>::AccountId,
			<T as Config>::AssetId,
			<T as Config>::Balance,
			BlockNumberFor<T>,
		>,
		<T as Config>::MaxPayoutsPerBounty,
	>;

	type SchedulesOf<T> = BoundedVec<
		VestingScheduleInfo<
			<T as Config>::AssetId,
			<T as Config>::Balance,
			BlockNumberFor<T>,
		>,
		<T as Config>::MaxSchedulesPerBounty,
	>;

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct Bounty<T: Config> {
		pub bounty_id: u64,
		pub creator: T::AccountId,
		pub judge: T::AccountId,
		pub metadata: BoundedMetadataOf<T>,
		pub status: BountyStatus<T::AccountId>,
		pub rewards: RewardsOf<T>,
		pub created_at: BlockNumberFor<T>,
		pub expires_at: Option<BlockNumberFor<T>>,
		pub payouts: PayoutsOf<T>,
	}

	#[pallet::storage]
	#[pallet::getter(fn next_bounty_id)]
	pub type NextBountyId<T: Config> = StorageValue<_, u64, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn bounties)]
	pub type Bounties<T: Config> = StorageMap<_, Blake2_128Concat, u64, Bounty<T>>;

	/// Applications per bounty (bounded vector)
	#[pallet::storage]
	#[pallet::getter(fn applications)]
	pub type Applications<T: Config> = StorageMap<_, Blake2_128Concat, u64, ApplicationsOf<T>, ValueQuery>;

	/// Per (account, bounty) vesting schedules (bounded)
	#[pallet::storage]
	#[pallet::getter(fn vesting_schedules)]
	pub type VestingSchedules<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat, T::AccountId,
		Blake2_128Concat, u64,
		SchedulesOf<T>,
		ValueQuery,
	>;

	/// Remaining (escrowed) amount per (bounty, asset). Drives payouts, finalization, refunds.
	#[pallet::storage]
	#[pallet::getter(fn remaining)]
	pub type RemainingByAsset<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat, u64,
		Blake2_128Concat, T::AssetId,
		T::Balance,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		BountyCreated { bounty_id: u64, creator: T::AccountId },
		FundsEscrowed { bounty_id: u64, asset: T::AssetId, amount: T::Balance },
		ApplicationSubmitted { bounty_id: u64, applicant: T::AccountId },
		BountyAssigned { bounty_id: u64, worker: T::AccountId },
		BountySetOpenToAll { bounty_id: u64 },
		DeliverySubmitted { bounty_id: u64, worker: T::AccountId },
		PartialPayout { bounty_id: u64, to: T::AccountId, asset: T::AssetId, amount: T::Balance, vesting: bool },
		BountyApproved { bounty_id: u64, worker: T::AccountId },
		BountyCancelled { bounty_id: u64 },
		BountyExpired { bounty_id: u64 },
		FundsReturned { bounty_id: u64, asset: T::AssetId, amount: T::Balance },
		VestingScheduled { bounty_id: u64, to: T::AccountId, asset: T::AssetId, amount: T::Balance },
		VestingClaimed { account: T::AccountId, bounty_id: u64, asset: T::AssetId, amount: T::Balance },
	}

	#[pallet::error]
	pub enum Error<T> {
		BountyNotFound,
		NotCreator,
		NotAuthorized,
		NotJudge,
		NotWorker,
		InvalidStatus,
		BountyExpired,
		AlreadyExpired,
		NotBeforeExpiry,
		TooManyRewards,
		TooManyApplications,
		TooManyPayouts,
		TooManySchedules,
		MetadataTooLong,
		ZeroAmount,
		InvalidVestingSchedule,
		NothingRemainingForAsset,
		AmountExceedsRemaining,
		ApplicationExists,
		NothingToClaim,
		DuplicateAssetInRewards,
	}

	// Weights trait (parameterize loops for accurate benchmarking later)
	pub trait WeightInfo {
		fn create_bounty(r: u32) -> Weight;
		fn apply() -> Weight;
		fn assign() -> Weight;
		fn set_open_to_all() -> Weight;
		fn deliver() -> Weight;
		fn approve_partial() -> Weight;
		fn approve_final(r: u32) -> Weight;
		fn cancel() -> Weight;
		fn expire() -> Weight;
		fn claim_vested(s: u32) -> Weight;
	}
	impl WeightInfo for () {
		fn create_bounty(_: u32) -> Weight { Weight::zero() }
		fn apply() -> Weight { Weight::zero() }
		fn assign() -> Weight { Weight::zero() }
		fn set_open_to_all() -> Weight { Weight::zero() }
		fn deliver() -> Weight { Weight::zero() }
		fn approve_partial() -> Weight { Weight::zero() }
		fn approve_final(_: u32) -> Weight { Weight::zero() }
		fn cancel() -> Weight { Weight::zero() }
		fn expire() -> Weight { Weight::zero() }
		fn claim_vested(_: u32) -> Weight { Weight::zero() }
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a bounty and escrow rewards into the pallet account.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::create_bounty(rewards.len() as u32))]
		pub fn create_bounty(
			origin: OriginFor<T>,
			judge: T::AccountId,
			metadata: Vec<u8>,
			rewards: Vec<Reward<T::AssetId, T::Balance, BlockNumberFor<T>>>,
			expires_at: Option<BlockNumberFor<T>>,
			open_to_all: bool,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;

			ensure!(!rewards.is_empty(), Error::<T>::TooManyRewards); // reuse error to avoid new one
			ensure!(rewards.len() as u32 <= T::MaxRewardsPerBounty::get(), Error::<T>::TooManyRewards);

			// Validate rewards and uniqueness
			let mut seen = BTreeSet::new();
			for r in rewards.iter() {
				ensure!(!r.amount.is_zero(), Error::<T>::ZeroAmount);
				ensure!(seen.insert(r.asset), Error::<T>::DuplicateAssetInRewards);
				if let Some(v) = &r.vesting {
					ensure!(v.count > 0, Error::<T>::InvalidVestingSchedule);
					ensure!(v.period > Zero::zero(), Error::<T>::InvalidVestingSchedule);
					if let Some(cliff) = v.cliff { ensure!(v.start <= cliff, Error::<T>::InvalidVestingSchedule); }
				}
			}

			let metadata: BoundedMetadataOf<T> = metadata.try_into().map_err(|_| Error::<T>::MetadataTooLong)?;

			let bounty_id = NextBountyId::<T>::get();
			let escrow = Self::account_id();

			// Transfer to escrow & set remaining per asset
			for r in rewards.iter() {
				<T as Config>::Currency::transfer(r.asset, &creator, &escrow, r.amount)?;
				RemainingByAsset::<T>::mutate(bounty_id, r.asset, |rem| *rem = rem.saturating_add(r.amount));
				Self::deposit_event(Event::FundsEscrowed { bounty_id, asset: r.asset, amount: r.amount });
			}

			let bounded_rewards: RewardsOf<T> = rewards.try_into().map_err(|_| Error::<T>::TooManyRewards)?;
			let status = if open_to_all { BountyStatus::OpenToAll } else { BountyStatus::Open };

			let bounty = Bounty::<T> {
				bounty_id,
				creator: creator.clone(),
				judge,
				metadata,
				status,
				rewards: bounded_rewards,
				created_at: <frame_system::Pallet<T>>::block_number(),
				expires_at,
				payouts: PayoutsOf::<T>::default(),
			};

			Bounties::<T>::insert(bounty_id, bounty);
			NextBountyId::<T>::put(bounty_id.saturating_add(1));

			Self::deposit_event(Event::BountyCreated { bounty_id, creator });
			Ok(())
		}

		/// Apply to a bounty (stores small metadata pointer)
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::apply())]
		pub fn apply(origin: OriginFor<T>, bounty_id: u64, metadata: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let mut apps = Applications::<T>::get(bounty_id);
			let b = Bounties::<T>::get(bounty_id).ok_or(Error::<T>::BountyNotFound)?;
			ensure!(matches!(b.status, BountyStatus::Open | BountyStatus::OpenToAll), Error::<T>::InvalidStatus);
			if let Some(exp) = b.expires_at { ensure!(<frame_system::Pallet<T>>::block_number() <= exp, Error::<T>::BountyExpired); }
			ensure!((apps.len() as u32) < T::MaxApplicationsPerBounty::get(), Error::<T>::TooManyApplications);
			ensure!(apps.iter().all(|a| a.applicant != who), Error::<T>::ApplicationExists);

			let m: BoundedMetadataOf<T> = metadata.try_into().map_err(|_| Error::<T>::MetadataTooLong)?;
			apps.try_push(Application { applicant: who.clone(), metadata: m, submitted_at: <frame_system::Pallet<T>>::block_number() })
				.map_err(|_| Error::<T>::TooManyApplications)?;
			Applications::<T>::insert(bounty_id, apps);

			Self::deposit_event(Event::ApplicationSubmitted { bounty_id, applicant: who });
			Ok(())
		}

		/// Assign a worker (creator or judge can assign)
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::assign())]
		pub fn assign(origin: OriginFor<T>, bounty_id: u64, worker: T::AccountId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Bounties::<T>::try_mutate(bounty_id, |mb| -> DispatchResult {
				let b = mb.as_mut().ok_or(Error::<T>::BountyNotFound)?;
				ensure!(who == b.creator || who == b.judge, Error::<T>::NotAuthorized);
				ensure!(matches!(b.status, BountyStatus::Open | BountyStatus::OpenToAll), Error::<T>::InvalidStatus);
				b.status = BountyStatus::Assigned { worker: worker.clone() };
				Ok(())
			})?;
			Self::deposit_event(Event::BountyAssigned { bounty_id, worker });
			Ok(())
		}

		/// Switch to open-to-all (creator or judge)
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::set_open_to_all())]
		pub fn set_open_to_all(origin: OriginFor<T>, bounty_id: u64) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Bounties::<T>::try_mutate(bounty_id, |mb| -> DispatchResult {
				let b = mb.as_mut().ok_or(Error::<T>::BountyNotFound)?;
				ensure!(who == b.creator || who == b.judge, Error::<T>::NotAuthorized);
				ensure!(matches!(b.status, BountyStatus::Open | BountyStatus::OpenToAll), Error::<T>::InvalidStatus);
				b.status = BountyStatus::OpenToAll;
				Ok(())
			})?;
			Self::deposit_event(Event::BountySetOpenToAll { bounty_id });
			Ok(())
		}

		/// Submit delivery
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::deliver())]
		pub fn deliver(origin: OriginFor<T>, bounty_id: u64) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Bounties::<T>::try_mutate(bounty_id, |mb| -> DispatchResult {
				let b = mb.as_mut().ok_or(Error::<T>::BountyNotFound)?;
				match &b.status {
					BountyStatus::Assigned { worker } => ensure!(&who == worker, Error::<T>::NotWorker),
					BountyStatus::Open | BountyStatus::OpenToAll => {},
					_ => return Err(Error::<T>::InvalidStatus.into()),
				}
				if let Some(exp) = b.expires_at { ensure!(<frame_system::Pallet<T>>::block_number() <= exp, Error::<T>::BountyExpired); }
				b.status = BountyStatus::Delivered(who.clone());
				Ok(())
			})?;
			Self::deposit_event(Event::DeliverySubmitted { bounty_id, worker: who });
			Ok(())
		}

		/// Approve partial payout. Optionally provide a vesting override for this slice.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::approve_partial())]
		pub fn approve_partial(
			origin: OriginFor<T>,
			bounty_id: u64,
			asset: T::AssetId,
			amount: T::Balance,
			vesting_override: Option<VestingSchedule<BlockNumberFor<T>>>,
		) -> DispatchResult {
			let judge = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);

			// Determine receiver and validate status/judge
			let (to, default_vesting) = {
				let b = Bounties::<T>::get(bounty_id).ok_or(Error::<T>::BountyNotFound)?;
				ensure!(b.judge == judge, Error::<T>::NotJudge);
				let to = match &b.status {
					BountyStatus::Delivered(w) | BountyStatus::PartiallyPaid(w) => w.clone(),
					BountyStatus::Assigned { worker } => worker.clone(),
					_ => return Err(Error::<T>::InvalidStatus.into()),
				};
				let v = b.rewards.iter().find(|r| r.asset == asset).and_then(|r| r.vesting.clone());
				(to, v)
			};

			// Remaining checks and decrement
			RemainingByAsset::<T>::try_mutate(bounty_id, asset, |rem| -> DispatchResult {
				if rem.is_zero() { return Err(Error::<T>::NothingRemainingForAsset.into()); }
				if amount > *rem { return Err(Error::<T>::AmountExceedsRemaining.into()); }
				*rem = rem.saturating_sub(amount);
				Ok(())
			})?;

			// Execute slice payout (immediate or vesting)
			let vesting_to_use = vesting_override.or(default_vesting);
			Self::process_payout_slice(bounty_id, &to, asset, amount, vesting_to_use)?;

			// Mark status
			Bounties::<T>::mutate(bounty_id, |mb| if let Some(b) = mb { b.status = BountyStatus::PartiallyPaid(to.clone()) });

			Ok(())
		}

		/// Approve final payout: pay remaining for each asset using default vesting.
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::approve_final(Bounties::<T>::get(bounty_id).map(|b| b.rewards.len() as u32).unwrap_or(0)))]
		pub fn approve_final(origin: OriginFor<T>, bounty_id: u64) -> DispatchResult {
			let judge = ensure_signed(origin)?;
			let (to, rewards) = {
				let b = Bounties::<T>::get(bounty_id).ok_or(Error::<T>::BountyNotFound)?;
				ensure!(b.judge == judge, Error::<T>::NotJudge);
				let to = match &b.status {
					BountyStatus::Delivered(w) | BountyStatus::PartiallyPaid(w) | BountyStatus::Assigned { worker: w } => w.clone(),
					_ => return Err(Error::<T>::InvalidStatus.into()),
				};
				(to, b.rewards.clone())
			};

			for r in rewards.iter() {
				let rem = RemainingByAsset::<T>::take(bounty_id, r.asset);
				if rem.is_zero() { continue; }
				Self::process_payout_slice(bounty_id, &to, r.asset, rem, r.vesting.clone())?;
			}

			Bounties::<T>::mutate(bounty_id, |mb| if let Some(b) = mb { b.status = BountyStatus::Approved(to.clone()) });
			Self::deposit_event(Event::BountyApproved { bounty_id, worker: to });
			Ok(())
		}

		/// Cancel bounty (creator only) before approval. Refund remaining to creator.
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::cancel())]
		pub fn cancel(origin: OriginFor<T>, bounty_id: u64) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let (creator, rewards, status) = {
				let b = Bounties::<T>::get(bounty_id).ok_or(Error::<T>::BountyNotFound)?;
				(b.creator, b.rewards, b.status)
			};
			ensure!(who == creator, Error::<T>::NotCreator);
			ensure!(matches!(status, BountyStatus::Open | BountyStatus::OpenToAll | BountyStatus::Assigned { .. } | BountyStatus::Delivered(_) | BountyStatus::PartiallyPaid(_)), Error::<T>::InvalidStatus);
			Self::refund_remaining_to_creator(bounty_id, &creator, &rewards)?;
			Bounties::<T>::mutate(bounty_id, |mb| if let Some(b) = mb { b.status = BountyStatus::Cancelled });
			Self::deposit_event(Event::BountyCancelled { bounty_id });
			Ok(())
		}

		/// Expire bounty (anyone) after expires_at, refund remaining to creator.
		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::expire())]
		pub fn expire(origin: OriginFor<T>, bounty_id: u64) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			let now = <frame_system::Pallet<T>>::block_number();
			let (creator, rewards, status, expiry) = {
				let b = Bounties::<T>::get(bounty_id).ok_or(Error::<T>::BountyNotFound)?;
				(b.creator, b.rewards, b.status, b.expires_at.ok_or(Error::<T>::AlreadyExpired)?)
			};
			ensure!(now > expiry, Error::<T>::NotBeforeExpiry);
			ensure!(matches!(status, BountyStatus::Open | BountyStatus::OpenToAll | BountyStatus::Assigned { .. } | BountyStatus::Delivered(_) | BountyStatus::PartiallyPaid(_)), Error::<T>::InvalidStatus);
			Self::refund_remaining_to_creator(bounty_id, &creator, &rewards)?;
			Bounties::<T>::mutate(bounty_id, |mb| if let Some(b) = mb { b.status = BountyStatus::Expired });
			Self::deposit_event(Event::BountyExpired { bounty_id });
			Ok(())
		}

		/// Claim vested amounts for a bounty (pull model).
		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::claim_vested(10))]
		pub fn claim_vested(origin: OriginFor<T>, bounty_id: u64) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let mut schedules = VestingSchedules::<T>::get(&who, bounty_id);
			ensure!(!schedules.is_empty(), Error::<T>::NothingToClaim);

			let mut any_claimed = false;
			for s in schedules.iter_mut() {
				let claimable = Self::calculate_claimable(s)?;
				if !claimable.is_zero() {
					<T as Config>::Currency::transfer(s.asset, &Self::account_id(), &who, claimable)?;
					s.claimed = s.claimed.saturating_add(claimable);
					any_claimed = true;
					Self::deposit_event(Event::VestingClaimed { account: who.clone(), bounty_id, asset: s.asset, amount: claimable });
				}
			}
			ensure!(any_claimed, Error::<T>::NothingToClaim);

			// Retain only schedules with remaining amounts
			schedules.retain(|s| s.claimed < s.total_amount);
			VestingSchedules::<T>::insert(&who, bounty_id, schedules);
			Ok(())
		}
	}

	// ------------------------------------
	// Helpers
	// ------------------------------------

	impl<T: Config> Pallet<T> {
		pub fn account_id() -> T::AccountId { T::PalletId::get().into_account_truncating() }

		fn refund_remaining_to_creator(
			bounty_id: u64,
			creator: &T::AccountId,
			rewards: &RewardsOf<T>,
		) -> DispatchResult {
			let escrow = Self::account_id();
			for r in rewards.iter() {
				let rem = RemainingByAsset::<T>::take(bounty_id, r.asset);
				if rem.is_zero() { continue; }
				<T as Config>::Currency::transfer(r.asset, &escrow, creator, rem)?;
				Self::deposit_event(Event::FundsReturned { bounty_id, asset: r.asset, amount: rem });
			}
			Ok(())
		}

		fn process_payout_slice(
			bounty_id: u64,
			to: &T::AccountId,
			asset: T::AssetId,
			amount: T::Balance,
			vesting: Option<VestingSchedule<BlockNumberFor<T>>>,
		) -> DispatchResult {
			let now = <frame_system::Pallet<T>>::block_number();
			let escrow = Self::account_id();

			// Track payout entry (bounded)
			Bounties::<T>::mutate(bounty_id, |mb| if let Some(b) = mb {
				let _ = b.payouts.try_push(Payout { to: to.clone(), asset, amount, at: now, vesting_applied: vesting.is_some() });
			});

			if let Some(v) = vesting {
				// Validate schedule (defensive)
				ensure!(v.count > 0, Error::<T>::InvalidVestingSchedule);
				ensure!(v.period > Zero::zero(), Error::<T>::InvalidVestingSchedule);
				if let Some(cliff) = v.cliff { ensure!(v.start <= cliff, Error::<T>::InvalidVestingSchedule); }

				// Internal vesting: keep funds in escrow; beneficiary will `claim_vested`
				let info = VestingScheduleInfo { asset, total_amount: amount, claimed: Zero::zero(), start: v.start, period: v.period, count: v.count, cliff: v.cliff, cliff_behavior: v.cliff_behavior };
				VestingSchedules::<T>::try_mutate(to, bounty_id, |vec| -> Result<(), DispatchError> {
					if (vec.len() as u32) >= T::MaxSchedulesPerBounty::get() { return Err(Error::<T>::TooManySchedules.into()); }
					vec.try_push(info).map_err(|_| Error::<T>::TooManySchedules.into())
				})?;
				Self::deposit_event(Event::VestingScheduled { bounty_id, to: to.clone(), asset, amount });
			} else {
				// Immediate transfer
				<T as Config>::Currency::transfer(asset, &escrow, to, amount)?;
				Self::deposit_event(Event::PartialPayout { bounty_id, to: to.clone(), asset, amount, vesting: false });
			}
			Ok(())
		}

		fn calculate_claimable(
			s: &VestingScheduleInfo<
				<T as Config>::AssetId,
				<T as Config>::Balance,
				BlockNumberFor<T>,
			>,
		) -> Result<T::Balance, DispatchError> {
			let now = <frame_system::Pallet<T>>::block_number();

			// Guard against zero values (should not happen due to validation)
			if s.count == 0 { return Err(Error::<T>::InvalidVestingSchedule.into()); }
			if s.period == Zero::zero() { return Err(Error::<T>::InvalidVestingSchedule.into()); }

			let per_period = s.total_amount / s.count.into();

			if let Some(cliff) = s.cliff {
				if now < cliff { return Ok(Zero::zero()); }
				match s.cliff_behavior {
					CliffBehavior::Linear => {
						let elapsed = now.saturating_sub(cliff);
						let periods = elapsed / s.period;
						let vested_periods: u32 = periods.try_into().unwrap_or(u32::MAX).min(s.count);
						let vested = per_period.saturating_mul(vested_periods.into());
						Ok(vested.saturating_sub(s.claimed))
					}
					CliffBehavior::CatchUp => {
						if now == cliff {
							let periods_until_cliff = (cliff.saturating_sub(s.start)) / s.period;
							let vested_periods: u32 = periods_until_cliff.try_into().unwrap_or(u32::MAX).min(s.count);
							let vested = per_period.saturating_mul(vested_periods.into());
							return Ok(vested.saturating_sub(s.claimed));
						}
						let total_periods = (now.saturating_sub(s.start)) / s.period;
						let vested_periods: u32 = total_periods.try_into().unwrap_or(u32::MAX).min(s.count);
						let vested = per_period.saturating_mul(vested_periods.into());
						Ok(vested.saturating_sub(s.claimed))
					}
				}
			} else {
				if now < s.start { return Ok(Zero::zero()); }
				let periods = (now.saturating_sub(s.start)) / s.period;
				let vested_periods: u32 = periods.try_into().unwrap_or(u32::MAX).min(s.count);
				let vested = per_period.saturating_mul(vested_periods.into());
				Ok(vested.saturating_sub(s.claimed))
			}
		}
	}
}
