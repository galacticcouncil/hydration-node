use crate::traits::AssetDepositLimiter;
use crate::types::LockdownStatus;
use crate::{AssetLockdownState, Config, Event, Pallet};
use frame_support::traits::Get;
use frame_system::pallet_prelude::BlockNumberFor;
use orml_traits::currency::OnDeposit;
use orml_traits::{GetByKey, Handler, Happened};
use sp_runtime::{SaturatedConversion, Saturating};
use sp_std::marker::PhantomData;

//TODO: check every he usage of saturaring

//TODO: add prop tests, also for save deposit, so only the claimed and specified amount is returned

//TODO: integration tests
// when other parachain is hacked, we should be able to lock down the asset issuance
// sending VDOT to us and exchange it
// Other parachain can send us VDOT, we exchange it for DOT and lock down the issuance
// example: byfrist xcm transfer to our asset crossing, so we should mint it, but we should not alllow
// Other test: replicate the problem we had last week, where you could mint any amount of sharetoken in stablepool
// --- we set limit for sharetoken in asset registry, when add liquidty, but when this cross this, it should reserve

pub struct IssuanceIncreaseFuse<T: Config>(PhantomData<T>);

impl<T: Config> OnDeposit<T::AccountId, T::AssetId, T::Balance> for IssuanceIncreaseFuse<T> {
	fn on_deposit(currency_id: T::AssetId, who: &T::AccountId, amount: T::Balance) -> sp_runtime::DispatchResult {
		let period = <T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::Period::get();
		if period == 0u128 {
			// no limit
			return Ok(());
		}

		let Some(limit) =
			<T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::DepositLimit::get(
				&currency_id,
			)
		else {
			return Ok(());
		};

		let current_block = <frame_system::Pallet<T>>::block_number();
		let lockdown_until: BlockNumberFor<T> = current_block.saturating_add(period.saturated_into());
		let asset_issuance =
			<T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::Issuance::get(
				&currency_id,
			);

		match evaluate_lockdown::<T>(currency_id, amount, asset_issuance, limit, current_block, period) {
			LockdownDecision::Lock { amount: to_lock } => {
				Pallet::<T>::do_lock_deposit(&who, currency_id, to_lock)?;

				// Only reapply lockdown if current state is not already Locked and valid
				match AssetLockdownState::<T>::get(currency_id) {
					Some(LockdownStatus::Locked(until)) if until > current_block => {}
					_ => {
						Pallet::<T>::do_lockdown_asset(currency_id, lockdown_until)?;
					}
				}
			}
			LockdownDecision::ResetLimit { amount: new_issuance } => {
				Pallet::<T>::do_reset_deposit_limits(currency_id, new_issuance)?;
			}
			LockdownDecision::LiftLockdown { amount: new_issuance } => {
				Pallet::<T>::do_lift_lockdown(currency_id, new_issuance)?;
			}
			LockdownDecision::NoAction => {}
		}

		Ok(())
	}
}

pub enum LockdownDecision<Balance> {
	Lock { amount: Balance },
	ResetLimit { amount: Balance },
	LiftLockdown { amount: Balance },
	NoAction,
}

pub fn evaluate_lockdown<T: Config>(
	currency_id: T::AssetId,
	amount: T::Balance,
	asset_issuance: T::Balance,
	limit: T::Balance,
	current_block: BlockNumberFor<T>,
	period: u128,
) -> LockdownDecision<T::Balance> {
	match AssetLockdownState::<T>::get(currency_id) {
		None => {
			if amount > limit {
				let to_lock = amount.saturating_sub(limit);

				LockdownDecision::Lock {
					amount: to_lock,
				}
			} else {
				LockdownDecision::ResetLimit { amount: amount }
			}
		}
		Some(LockdownStatus::Locked(until)) if until > current_block => {
			LockdownDecision::Lock { amount }
		}
		Some(LockdownStatus::Locked(_)) => {
			// Lockdown expired
			// Check if this new deposit does not exceed the limit and lock it down again if it does.
			if amount > limit {
				LockdownDecision::Lock {
					amount: amount.saturating_sub(limit),
				}
			} else {
				LockdownDecision::LiftLockdown { amount: amount }
			}
		}
		Some(LockdownStatus::Unlocked((last_reset_at, _)))
		if last_reset_at.saturating_add(period.saturated_into()) <= current_block =>
			{
				// The period is over, so we can reset the limit.
				// But first, we must check if this new deposit does not exceed the limit.
				if amount > limit {
					LockdownDecision::Lock {
						amount: amount.saturating_sub(limit),
					}
				} else {
					LockdownDecision::ResetLimit { amount: amount }
				}
			}
		Some(LockdownStatus::Unlocked((_, last_issuance))) => {
			// If the period is not over, we check the limit by comparing issuance increase.
			let issued = asset_issuance.saturating_sub(last_issuance);
			if issued > limit {
				// We should lock only the excess, not all new deposit
				// Formula: to_lock = current_issuance - (last_issuance + limit)
				let to_lock = asset_issuance.saturating_sub(last_issuance.saturating_add(limit));
				debug_assert!(to_lock <= amount);
				LockdownDecision::Lock { amount: to_lock }
			} else {
				LockdownDecision::NoAction
			}
		}
	}
}