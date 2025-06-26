use crate::traits::AssetDepositLimiter;
use crate::types::AssetLockdownState;
use crate::{Config, LastAssetLockdownState, Pallet};
use frame_support::traits::Get;
use frame_system::pallet_prelude::BlockNumberFor;
use orml_traits::currency::OnDeposit;
use orml_traits::{GetByKey, Handler, Happened};
use sp_runtime::{SaturatedConversion, Saturating};
use sp_std::marker::PhantomData;
//TODO: EVENTS: when goes lockdw, when goes unclocke

//TODO: burned tokens deduct from limit to prevent DOS

//TODO: consider the solution for HDX too as that it is not involved in orml tokens

//TODO: CREATE ISSUE I guess if the supply increased (or decreased ) and the token is type external it was bridged in or out.
//TODO: in the trehad Jakub mentios that we need to handle burn too

//TODO:
//Limit thresholds should be calibrated based on bug bounty values, making it always more profitable for a hacker to report the issue than to exploit it.

//TODO: integration tests
// when other parachain is hacked, we should be able to lock down the asset issuance
// sending VDOT to us and exchange it
// Other parachain can send us VDOT, we exchange it for DOT and lock down the issuance
// example: byfrist xcm transfer to our asset crossing, so we should mint it, but we should not alllow
// Other test: replicate the problem we had last week, where you could mint any amount of sharetoken in stablepool
// --- we set limit for sharetoken in asset registry, when add liquidty, but when this cross this, it should reserve

//TODO: add integration test when there is no limit, so it should not lock down the asset issuance
pub struct IssuanceIncreaseFuse<T: Config>(PhantomData<T>);

impl<T: Config> OnDeposit<T::AccountId, T::AssetId, T::Balance> for IssuanceIncreaseFuse<T> {
	fn on_deposit(currency_id: T::AssetId, who: &T::AccountId, amount: T::Balance) -> sp_runtime::DispatchResult {
		let period = <T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::Period::get();
		if period == 0u128 {
			// no limit
			return Ok(());
		}
		let asset_issuance =
			<T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::Issuance::get(
				&currency_id,
			);

		let Some(limit) =
			<T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::DepositLimit::get(
				&currency_id,
			)
		else {
			// TODO: LOG
			return Ok(());
		};

		let current_block = <frame_system::Pallet<T>>::block_number();
		let Some(asset_state) = LastAssetLockdownState::<T>::get(currency_id) else {
			// Only when nothing is yet set for the asset
			// in this case - we need to store issuance with the first deposit - so it counts towards the limit on next deposit

			//If first deposit exceeds the limit, we lock it down, otherwise we store the issuance
			if amount > limit {
				let current_block: u128 = <frame_system::Pallet<T>>::block_number().saturated_into();
				let lockdown_until: BlockNumberFor<T> = current_block.saturating_add(period).saturated_into();
				LastAssetLockdownState::<T>::insert(currency_id, AssetLockdownState::Locked(lockdown_until));
				<T::DepositLimiter as AssetDepositLimiter<T::AccountId,T::AssetId, T::Balance>>::OnLimitReached::happened(&(currency_id));
				let to_lock = amount.saturating_sub(limit);
				<T::DepositLimiter as AssetDepositLimiter<T::AccountId,T::AssetId, T::Balance>>::OnLockdownDeposit::handle(&(currency_id, who.clone(), to_lock))?;
			} else {
				LastAssetLockdownState::<T>::insert(
					currency_id,
					AssetLockdownState::Unlocked((
						<frame_system::Pallet<T>>::block_number(),
						asset_issuance.saturating_sub(amount),
					)),
				);
			}

			return Ok(());
		};

		match asset_state {
			AssetLockdownState::Locked(until) => {
				if until > current_block {
					// lockdown still active
					//TODO: what we were intended to do here, only this?
					<T::DepositLimiter as AssetDepositLimiter<T::AccountId,T::AssetId, T::Balance>>::OnLockdownDeposit::handle(&(currency_id, who.clone(), amount))?;
				} else {
					// lockdown expired
					//TODO: but we dont have the previous issuance
					//TODO: try to break this  - Clarify: check only the amount against the limit ??!
					if amount > limit {
						let current_block: u128 = <frame_system::Pallet<T>>::block_number().saturated_into();
						let lockdown_until: BlockNumberFor<T> = current_block.saturating_add(period).saturated_into();
						LastAssetLockdownState::<T>::insert(currency_id, AssetLockdownState::Locked(lockdown_until));
						<T::DepositLimiter as AssetDepositLimiter<T::AccountId,T::AssetId, T::Balance>>::OnLimitReached::happened(&(currency_id));
						let to_lock = amount.saturating_sub(limit);
						<T::DepositLimiter as AssetDepositLimiter<T::AccountId,T::AssetId, T::Balance>>::OnLockdownDeposit::handle(&(currency_id, who.clone(), to_lock))?;
					} else {
						LastAssetLockdownState::<T>::insert(
							currency_id,
							AssetLockdownState::Unlocked((
								<frame_system::Pallet<T>>::block_number(),
								asset_issuance.saturating_sub(amount),
							)),
						);
					}
				}
			}
			AssetLockdownState::Unlocked((last_block, last_issuance)) => {
				let last_block: u128 = last_block.saturated_into();
				let current_block: u128 = <frame_system::Pallet<T>>::block_number().saturated_into();
				if last_block.saturating_add(period) <= current_block {
					// The period is over, so we can reset the limit.
					// But first, we must check if this new deposit on its own exceeds the limit.
					if amount > limit {
						// This single deposit exceeds the limit, lock it down.
						let lockdown_until: BlockNumberFor<T> = current_block.saturating_add(period).saturated_into();
						LastAssetLockdownState::<T>::insert(currency_id, AssetLockdownState::Locked(lockdown_until));

						<T::DepositLimiter as AssetDepositLimiter<T::AccountId,T::AssetId, T::Balance>>::OnLimitReached::happened(&(currency_id));
						let to_lock = amount.saturating_sub(limit);
						<T::DepositLimiter as AssetDepositLimiter<T::AccountId,T::AssetId, T::Balance>>::OnLockdownDeposit::handle(&(currency_id, who.clone(), to_lock))?;
					} else {
						// The deposit is fine, reset the baseline for the new period.
						let asset_issuance = <T::DepositLimiter as AssetDepositLimiter<
							T::AccountId,
							T::AssetId,
							T::Balance,
						>>::Issuance::get(&currency_id);
						LastAssetLockdownState::<T>::insert(
							currency_id,
							AssetLockdownState::Unlocked((
								<frame_system::Pallet<T>>::block_number(),
								asset_issuance.saturating_sub(amount), // Set new baseline
							)),
						);
					}
				} else {
					// If the period is not over, we check the limit
					let issuance_increase_in_period = asset_issuance.saturating_sub(last_issuance); //TODO: check every he usage of saturaring
					if issuance_increase_in_period > limit {
						// we reached the limit
						let lockdown_until: BlockNumberFor<T> = current_block.saturating_add(period).saturated_into();
						LastAssetLockdownState::<T>::insert(currency_id, AssetLockdownState::Locked(lockdown_until));

						<T::DepositLimiter as AssetDepositLimiter<T::AccountId,T::AssetId, T::Balance>>::OnLimitReached::happened(&(currency_id));
						// We should lock only difference here, not all new deposit
						// that's : Ok for last + issuance + limit, the rest is lockeddown.
						let to_lock = asset_issuance.saturating_sub(last_issuance.saturating_add(limit));
						<T::DepositLimiter as AssetDepositLimiter<T::AccountId,T::AssetId, T::Balance>>::OnLockdownDeposit::handle(&(currency_id, who.clone(), to_lock))?;
					}
				}
			}
		}

		Ok(())
	}
}
