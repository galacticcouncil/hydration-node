// This file is part of galacticcouncil/warehouse.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;

use hydradx_traits::liquidity_mining::PriceAdjustment;
pub use hydradx_traits::liquidity_mining::{DepositId, GlobalFarmId, YieldFarmId};

pub type FarmId = u32;
pub type Balance = u128;
pub type FarmMultiplier = FixedU128;

/// Default implementation of `PriceAdjustment` trait which returns `price_adjustment` value saved
/// in `GlobalFarm`.
pub struct DefaultPriceAdjustment;

impl<T: Config<I>, I: 'static> PriceAdjustment<GlobalFarmData<T, I>> for DefaultPriceAdjustment {
	type Error = DispatchError;

	type PriceAdjustment = FixedU128;

	fn get(global_farm: &GlobalFarmData<T, I>) -> Result<Self::PriceAdjustment, Self::Error> {
		Ok(global_farm.price_adjustment)
	}
}

/// This struct represents the state a of single liquidity mining program. `YieldFarm`s are rewarded from
/// `GlobalFarm` based on their stake in `GlobalFarm`. `YieldFarm` stake in `GlobalFarm` is derived from
/// users stake in `YieldFarm`.
/// Yield farm is considered live from global farm view if yield farm is `active` or `stopped`.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[codec(mel_bound())]
#[scale_info(skip_type_params(T, I))]
pub struct GlobalFarmData<T: Config<I>, I: 'static = ()> {
	pub id: GlobalFarmId,
	pub(super) owner: T::AccountId,
	pub(super) updated_at: PeriodOf<T>,
	pub(super) total_shares_z: Balance,
	pub(super) accumulated_rpz: FixedU128,
	pub reward_currency: T::AssetId,
	pub(super) pending_rewards: Balance,
	pub(super) accumulated_paid_rewards: Balance,
	pub(super) yield_per_period: Perquintill,
	pub(super) planned_yielding_periods: PeriodOf<T>,
	pub(super) blocks_per_period: BlockNumberFor<T>,
	pub incentivized_asset: T::AssetId,
	pub(super) max_reward_per_period: Balance,
	// min. LP shares user must deposit to start yield farming.
	pub(super) min_deposit: Balance,
	// This include `active` and `stopped` yield farms.
	pub(super) live_yield_farms_count: u32,
	// This include `active`, `stopped`, `terminated` - this count is decreased only if yield
	// farm is removed from storage.
	pub(super) total_yield_farms_count: u32,
	pub(super) price_adjustment: FixedU128,
	pub(super) state: FarmState,
}

impl<T: Config<I>, I: 'static> GlobalFarmData<T, I> {
	#[allow(clippy::too_many_arguments)]
	pub fn new(
		id: GlobalFarmId,
		updated_at: PeriodOf<T>,
		reward_currency: T::AssetId,
		yield_per_period: Perquintill,
		planned_yielding_periods: PeriodOf<T>,
		blocks_per_period: BlockNumberFor<T>,
		owner: T::AccountId,
		incentivized_asset: T::AssetId,
		max_reward_per_period: Balance,
		min_deposit: Balance,
		price_adjustment: FixedU128,
	) -> Self {
		Self {
			pending_rewards: Zero::zero(),
			accumulated_rpz: Zero::zero(),
			accumulated_paid_rewards: Zero::zero(),
			total_shares_z: Zero::zero(),
			live_yield_farms_count: Zero::zero(),
			total_yield_farms_count: Zero::zero(),
			id,
			updated_at,
			reward_currency,
			yield_per_period,
			planned_yielding_periods,
			blocks_per_period,
			owner,
			incentivized_asset,
			max_reward_per_period,
			min_deposit,
			price_adjustment,
			state: FarmState::Active,
		}
	}

	/// This function updates yields_farm_count when new yield farm is added into the global farm.
	/// This function should be called only when new yield farm is created/added into the global
	/// farm.
	pub fn increase_yield_farm_counts(&mut self) -> Result<(), ArithmeticError> {
		self.live_yield_farms_count = self
			.live_yield_farms_count
			.checked_add(1)
			.ok_or(ArithmeticError::Overflow)?;

		self.total_yield_farms_count = self
			.total_yield_farms_count
			.checked_add(1)
			.ok_or(ArithmeticError::Overflow)?;

		Ok(())
	}

	/// This function updates `yield_farms_count` when yield farm is terminated from global farm.
	/// This function should be called only when yield farm is removed from global farm.
	pub fn decrease_live_yield_farm_count(&mut self) -> Result<(), Error<T, I>> {
		//NOTE: only live count should change
		//NOTE: this counter is managed only by pallet so this sub should never fail.
		self.live_yield_farms_count = self
			.live_yield_farms_count
			.checked_sub(1)
			.defensive_ok_or::<Error<T, I>>(InconsistentStateError::InvalidLiveYielFarmsCount.into())?;

		Ok(())
	}

	/// This function updates `yield_farms_count` when yield farm is removed from storage.
	/// This function should be called only if yield farm was removed from storage.
	/// !!! DON'T call this function if yield farm is in stopped or terminated.
	pub fn decrease_total_yield_farm_count(&mut self) -> Result<(), Error<T, I>> {
		//NOTE: this counter is managed only by pallet so this sub should never fail.
		self.total_yield_farms_count = self
			.total_yield_farms_count
			.checked_sub(1)
			.defensive_ok_or::<Error<T, I>>(InconsistentStateError::InvalidTotalYieldFarmsCount.into())?;

		Ok(())
	}

	/// Function returns `true` if global farm has live yield farms.
	pub fn has_live_farms(&self) -> bool {
		!self.live_yield_farms_count.is_zero()
	}

	/// Function return `true` if global farm can be removed from storage.
	pub fn can_be_removed(&self) -> bool {
		//farm can be removed from storage only if all yield farms was removed from storage.
		self.state == FarmState::Terminated && self.total_yield_farms_count.is_zero()
	}

	/// This function returns `true` if farm has no capacity for next yield farm(yield farm can't
	/// be added into global farm until some yield farm is removed from storage).
	pub fn is_full(&self) -> bool {
		self.total_yield_farms_count.ge(&<T>::MaxYieldFarmsPerGlobalFarm::get())
	}

	/// Function adds `amount` to `total_shares_z`.
	pub fn add_stake(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_shares_z = self
			.total_shares_z
			.checked_add(amount)
			.ok_or(ArithmeticError::Overflow)?;

		Ok(())
	}

	/// Function removes `amount` from `total_shares_z`.
	pub fn remove_stake(&mut self, amount: Balance) -> Result<(), Error<T, I>> {
		//NOTE: This should never fail, we should never sub more than current state.
		self.total_shares_z = self
			.total_shares_z
			.checked_sub(amount)
			.defensive_ok_or::<Error<T, I>>(InconsistentStateError::InvalidTotalSharesZ.into())?;

		Ok(())
	}
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[codec(mel_bound())]
#[scale_info(skip_type_params(T, I))]
pub struct YieldFarmData<T: Config<I>, I: 'static = ()> {
	pub(super) id: FarmId,
	pub(super) updated_at: PeriodOf<T>,
	pub(super) total_shares: Balance,
	pub(super) total_valued_shares: Balance,
	pub(super) accumulated_rpvs: FixedU128,
	pub(super) accumulated_rpz: FixedU128,
	pub(super) loyalty_curve: Option<LoyaltyCurve>,
	pub(super) multiplier: FarmMultiplier,
	pub(super) state: FarmState,
	pub(super) entries_count: u64,
	pub(super) left_to_distribute: Balance,
	pub(super) total_stopped: PeriodOf<T>,
	pub(super) _phantom: PhantomData<I>,
}

impl<T: Config<I>, I: 'static> YieldFarmData<T, I> {
	#[allow(clippy::too_many_arguments)]
	pub fn new(
		id: FarmId,
		updated_at: PeriodOf<T>,
		loyalty_curve: Option<LoyaltyCurve>,
		multiplier: FarmMultiplier,
	) -> Self {
		Self {
			id,
			updated_at,
			loyalty_curve,
			multiplier,
			accumulated_rpvs: Zero::zero(),
			accumulated_rpz: Zero::zero(),
			total_shares: Zero::zero(),
			total_valued_shares: Zero::zero(),
			state: FarmState::Active,
			entries_count: Default::default(),
			left_to_distribute: Default::default(),
			total_stopped: Default::default(),
			_phantom: PhantomData,
		}
	}

	/// Returns `true` if yield farm can be removed from storage, `false` otherwise.
	pub fn can_be_removed(&self) -> bool {
		self.state == FarmState::Terminated && self.entries_count.is_zero()
	}

	/// This function updates entries count in the yield farm. This function should be called if  
	/// entry is removed from the yield farm.
	pub fn decrease_entries_count(&mut self) -> Result<(), Error<T, I>> {
		//NOTE: this counter is managed only by pallet so this sub should never fail.
		self.entries_count = self
			.entries_count
			.checked_sub(1)
			.defensive_ok_or::<Error<T, I>>(InconsistentStateError::InvalidYieldFarmEntriesCount.into())?;

		Ok(())
	}

	/// This function updates entries count in the yield farm. This function should be called if
	/// entry is added into the yield farm.
	pub fn increase_entries_count(&mut self) -> Result<(), ArithmeticError> {
		self.entries_count = self.entries_count.checked_add(1).ok_or(ArithmeticError::Overflow)?;

		Ok(())
	}

	/// This function return `true` if deposit exists in the yield farm.
	pub fn has_entries(&self) -> bool {
		!self.entries_count.is_zero()
	}
}

/// Loyalty curve to calculate loyalty multiplier.
///
/// `t = t_now - t_added`
/// `num = t + initial_reward_percentage * scale_coef`
/// `denom = t + scale_coef`
///
/// `loyalty_multiplier = num/denom`
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T, I))]
pub struct LoyaltyCurve {
	pub initial_reward_percentage: FixedU128,
	pub scale_coef: u32,
}

impl Default for LoyaltyCurve {
	fn default() -> Self {
		Self {
			initial_reward_percentage: FixedU128::from_inner(500_000_000_000_000_000), // 0.5
			scale_coef: 100,
		}
	}
}

/// Deposit represents a group of locked LP shares in the liquidity mining program("Position").
/// LP shares in the deposit can be locked in one or more yield farms based on pallet's
/// configuration(`MaxEntriesPerDeposit`).
/// The LP token's lock in the deposit is called "farm entry". Farm entry entitles deposit
/// owner to accumulate rewards from the yield farm.
/// Every deposit should have at least one farm entry and deposit without farm entries
/// should be removed from storage and LP shares should be unlocked.
/// `redeposit_lp_shares()` is used to add a new farm entry into the deposit("re-lock" LP shares").
#[derive(Clone, Encode, Decode, RuntimeDebug, TypeInfo, PartialEq, Eq, MaxEncodedLen)]
#[codec(mel_bound())]
#[scale_info(skip_type_params(T, I))]
pub struct DepositData<T: Config<I>, I: 'static = ()> {
	pub(super) shares: Balance,
	pub(super) amm_pool_id: T::AmmPoolId,
	// NOTE: `MaxFarmEntriesPerDeposit` of this vector MUST BE at least 1 for the configuration to
	// be sensible.
	pub(super) yield_farm_entries: BoundedVec<YieldFarmEntry<T, I>, T::MaxFarmEntriesPerDeposit>,
}

impl<T: Config<I>, I: 'static> DepositData<T, I> {
	pub fn new(shares: Balance, amm_pool_id: T::AmmPoolId) -> Self {
		Self {
			shares,
			amm_pool_id,
			yield_farm_entries: BoundedVec::default(),
		}
	}

	/// This function add new yield farm entry into the deposit.
	/// This function returns error if deposit reached max entries in the deposit or
	/// `entry.yield_farm_id` is not unique.
	pub fn add_yield_farm_entry(&mut self, entry: YieldFarmEntry<T, I>) -> Result<(), DispatchError> {
		if self.search_yield_farm_entry(entry.yield_farm_id).is_some() {
			return Err(Error::<T, I>::DoubleLock.into());
		}

		self.yield_farm_entries
			.try_push(entry)
			.map_err(|_| Error::<T, I>::MaxEntriesPerDeposit)?;

		Ok(())
	}

	/// This function remove yield farm entry from the deposit. This function returns error if
	/// yield farm entry in not found in the deposit.
	pub fn remove_yield_farm_entry(&mut self, yield_farm_id: YieldFarmId) -> Result<YieldFarmEntry<T, I>, Error<T, I>> {
		if let Some(idx) = self.search_yield_farm_entry(yield_farm_id) {
			return Ok(self.yield_farm_entries.swap_remove(idx));
		}

		Err(Error::<T, I>::YieldFarmEntryNotFound)
	}

	/// This function return yield farm entry from deposit of `None` if yield farm entry is not
	/// found.
	pub fn get_yield_farm_entry(&mut self, yield_farm_id: YieldFarmId) -> Option<&mut YieldFarmEntry<T, I>> {
		if let Some(idx) = self.search_yield_farm_entry(yield_farm_id) {
			return self.yield_farm_entries.get_mut(idx);
		}

		None
	}

	/// This function returns its index if deposit contains yield farm entry with given yield farm id.
	pub fn search_yield_farm_entry(&self, yield_farm_id: YieldFarmId) -> Option<usize> {
		self.yield_farm_entries
			.iter()
			.position(|e| e.yield_farm_id == yield_farm_id)
	}

	/// This function returns `true` if deposit can be removed from storage.
	pub fn can_be_removed(&self) -> bool {
		//NOTE: deposit with no entries should/must be removed from storage
		self.yield_farm_entries.is_empty()
	}
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[codec(mel_bound())]
#[scale_info(skip_type_params(T, I))]
pub struct YieldFarmEntry<T: Config<I>, I: 'static = ()> {
	pub(super) global_farm_id: GlobalFarmId,
	pub(super) yield_farm_id: YieldFarmId,
	pub(super) valued_shares: Balance,
	pub(super) accumulated_rpvs: FixedU128,
	pub(super) accumulated_claimed_rewards: Balance,
	pub(super) entered_at: PeriodOf<T>,
	pub(super) updated_at: PeriodOf<T>,
	// Number of periods yield-farm experienced before creation of this entry.
	pub(super) stopped_at_creation: PeriodOf<T>,
	pub(super) _phantom: PhantomData<I>,
}

impl<T: Config<I>, I: 'static> YieldFarmEntry<T, I> {
	pub fn new(
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		valued_shares: Balance,
		accumulated_rpvs: FixedU128,
		entered_at: PeriodOf<T>,
		stopped_at_creation: PeriodOf<T>,
	) -> Self {
		Self {
			global_farm_id,
			yield_farm_id,
			valued_shares,
			accumulated_rpvs,
			accumulated_claimed_rewards: Zero::zero(),
			entered_at,
			updated_at: entered_at,
			stopped_at_creation,
			_phantom: PhantomData,
		}
	}
}

/// An enum whose variants represent the state of the yield or global farm.
/// - `Active` - farm has full functionality. This state may be used for both farm types.
/// - `Stopped` - only partial functionality of the farm is available to users. Farm can became
/// `Active` again or can be `Terminated`. This state can be used only for yield farms.
/// - `Terminated` - farm is destroyed and it's waiting to be removed from the storage. This state can't be
/// reverted and is available for both farm types.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum FarmState {
	Active,
	Stopped,
	Terminated,
}

impl FarmState {
	pub fn is_active(&self) -> bool {
		*self == FarmState::Active
	}
	pub fn is_stopped(&self) -> bool {
		*self == FarmState::Stopped
	}
	pub fn is_terminated(&self) -> bool {
		*self == FarmState::Terminated
	}
}
