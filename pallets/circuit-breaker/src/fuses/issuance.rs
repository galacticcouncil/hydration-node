use crate::traits::AssetDepositLimiter;
use crate::types::AssetLockdownState;
use crate::{Config, LastAssetIssuance, Pallet};
use frame_support::traits::Get;
use frame_system::pallet_prelude::BlockNumberFor;
use orml_traits::currency::OnDeposit;
use orml_traits::{GetByKey, Handler, Happened};
use sp_runtime::{SaturatedConversion, Saturating};
use std::marker::PhantomData;

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
		let current_block = <frame_system::Pallet<T>>::block_number();
		let Some(asset_state) = LastAssetIssuance::<T>::get(currency_id) else {
			// Only when nothing is yet set for the asset
			// in this case - we need to store issuance with the first deposit - so it counts towares the limit on next deposits
			// TODO: check if this fist deposit does not exceed the limit
			LastAssetIssuance::<T>::insert(
				currency_id,
				AssetLockdownState::Unlocked((
					<frame_system::Pallet<T>>::block_number(),
					asset_issuance.saturating_sub(amount),
				)),
			);
			return Ok(());
		};

		match asset_state {
			AssetLockdownState::Locked(until) => {
				// asset on lockdown
				// first check if expired
				if until > current_block {
					// lockdown still active
					<T::DepositLimiter as AssetDepositLimiter<T::AccountId,T::AssetId, T::Balance>>::OnLockdownDeposit::handle(&(currency_id, who.clone(), amount))?;
				} else {
					// lockdown expired
					// we still need to  check that this deposit has not exceeded the limit
					//TODO: but we dont have the previous issuance
					//Clarify: check only the amount against the limit ??!
					let limit = <T::DepositLimiter as AssetDepositLimiter<T::AccountId,T::AssetId, T::Balance>>::DepositLimit::get(&currency_id);
					if amount > limit {
						let current_block: u128 = <frame_system::Pallet<T>>::block_number().saturated_into();
						let lockdown_until: BlockNumberFor<T> = current_block.saturating_add(period).saturated_into();
						LastAssetIssuance::<T>::insert(currency_id, AssetLockdownState::Locked(lockdown_until));
					} else {
						LastAssetIssuance::<T>::insert(
							currency_id,
							AssetLockdownState::Unlocked((<frame_system::Pallet<T>>::block_number(), asset_issuance)),
						);
					}
				}
			}
			AssetLockdownState::Unlocked((last_block, last_issuance)) => {
				let last_block: u128 = last_block.saturated_into();
				let current_block: u128 = <frame_system::Pallet<T>>::block_number().saturated_into();
				if last_block.saturating_add(period) <= current_block {
					// we can reset the limit
					// the period is over and limit was not reached
					//TODO: we should also check this deposit against the limit
					LastAssetIssuance::<T>::insert(
						currency_id,
						AssetLockdownState::Unlocked((<frame_system::Pallet<T>>::block_number(), asset_issuance)),
					);
				} else {
					// If the period is not over, we check the limit
					let limit = <T::DepositLimiter as AssetDepositLimiter<T::AccountId,T::AssetId, T::Balance>>::DepositLimit::get(&currency_id);
					let issuance_difference = asset_issuance.saturating_sub(last_issuance);
					if issuance_difference > limit {
						// we reached the limit
						let lockdown_until: BlockNumberFor<T> = current_block.saturating_add(period).saturated_into();
						LastAssetIssuance::<T>::insert(currency_id, AssetLockdownState::Locked(lockdown_until));

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
