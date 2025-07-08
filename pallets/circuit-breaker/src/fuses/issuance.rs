use crate::traits::AssetDepositLimiter;
use crate::types::LockdownStatus;
use crate::{AssetLockdownState, Config, Pallet};
use frame_support::traits::Get;
use frame_system::pallet_prelude::BlockNumberFor;
use orml_traits::currency::OnDeposit;
use orml_traits::GetByKey;
use sp_runtime::{SaturatedConversion, Saturating};
use sp_std::marker::PhantomData;

//TODO: check every he usage of saturaring

//TODO: add prop tests, also for save deposit, so only the claimed and specified amount is returned

#[derive(Debug)]
enum DepositAction<Balance> {
	InitialDeposit,
	LockdownActive,
	LockdownExpired,
	PeriodExpired,
	WithinPeriod { last_issuance: Balance },
}

struct DepositContext<T: Config> {
	action: DepositAction<T::Balance>,
	limit: T::Balance,
	lockdown_until: BlockNumberFor<T>,
	asset_issuance: T::Balance,
}

pub struct IssuanceIncreaseFuse<T: Config>(PhantomData<T>);

impl<T: Config> IssuanceIncreaseFuse<T> {
	/// Check if the given amount can be minted for the asset
	pub fn can_mint(currency_id: T::AssetId, amount: T::Balance) -> bool {
		let Some(context) = Self::get_context(currency_id) else {
			return true;
		};

		match &context.action {
			DepositAction::LockdownActive => false,
			DepositAction::InitialDeposit | DepositAction::LockdownExpired | DepositAction::PeriodExpired => {
				amount <= context.limit
			}
			DepositAction::WithinPeriod { last_issuance } => {
				let issuance_increase_in_period = context.asset_issuance.saturating_sub(*last_issuance);
				issuance_increase_in_period <= context.limit
			}
		}
	}

	fn get_context(currency_id: T::AssetId) -> Option<DepositContext<T>> {
		let period = <T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::Period::get();
		if period == 0u128 {
			return None;
		}

		let Some(limit) =
			<T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::DepositLimit::get(
				&currency_id,
			)
		else {
			return None;
		};

		let current_block = <frame_system::Pallet<T>>::block_number();
		let lockdown_until = current_block.saturating_add(period.saturated_into());
		let asset_issuance =
			<T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::Issuance::get(
				&currency_id,
			);

		let state = AssetLockdownState::<T>::get(currency_id);
		let action = Self::classify_state(state, current_block, period);

		Some(DepositContext {
			action,
			limit,
			lockdown_until,
			asset_issuance,
		})
	}

	fn classify_state(
		state: Option<LockdownStatus<BlockNumberFor<T>, T::Balance>>,
		current_block: BlockNumberFor<T>,
		period: u128,
	) -> DepositAction<T::Balance> {
		match state {
			None => DepositAction::InitialDeposit,
			Some(LockdownStatus::Locked(until)) if until > current_block => DepositAction::LockdownActive,
			Some(LockdownStatus::Locked(_)) => DepositAction::LockdownExpired,
			Some(LockdownStatus::Unlocked((last_reset_at, last_issuance))) => {
				if last_reset_at.saturating_add(period.saturated_into()) <= current_block {
					DepositAction::PeriodExpired
				} else {
					DepositAction::WithinPeriod { last_issuance }
				}
			}
		}
	}

	fn process_deposit_action(
		context: &DepositContext<T>,
		who: &T::AccountId,
		currency_id: T::AssetId,
		amount: T::Balance,
	) -> sp_runtime::DispatchResult {
		match &context.action {
			DepositAction::InitialDeposit | DepositAction::PeriodExpired => {
				Self::handle_limit_reset(context, who, currency_id, amount)
			}
			DepositAction::LockdownExpired => Self::handle_lockdown_expired(context, who, currency_id, amount),
			DepositAction::LockdownActive => Pallet::<T>::do_lock_deposit(who, currency_id, amount),
			DepositAction::WithinPeriod { last_issuance } => {
				Self::handle_within_period(context, who, currency_id, amount, *last_issuance)
			}
		}
	}

	fn handle_limit_reset(
		context: &DepositContext<T>,
		who: &T::AccountId,
		currency_id: T::AssetId,
		amount: T::Balance,
	) -> sp_runtime::DispatchResult {
		if amount > context.limit {
			let to_lock = amount.saturating_sub(context.limit);
			Pallet::<T>::do_lock_deposit(who, currency_id, to_lock)?;
			Pallet::<T>::do_lockdown_asset(currency_id, context.lockdown_until)?;
		} else {
			Pallet::<T>::do_reset_deposit_limits(currency_id, amount)?;
		}
		Ok(())
	}

	fn handle_lockdown_expired(
		context: &DepositContext<T>,
		who: &T::AccountId,
		currency_id: T::AssetId,
		amount: T::Balance,
	) -> sp_runtime::DispatchResult {
		if amount > context.limit {
			let to_lock = amount.saturating_sub(context.limit);
			Pallet::<T>::do_lock_deposit(who, currency_id, to_lock)?;
			Pallet::<T>::do_lockdown_asset(currency_id, context.lockdown_until)?;
		} else {
			Pallet::<T>::do_lift_lockdown(currency_id, amount)?;
		}
		Ok(())
	}

	fn handle_within_period(
		context: &DepositContext<T>,
		who: &T::AccountId,
		currency_id: T::AssetId,
		_amount: T::Balance,
		last_issuance: T::Balance,
	) -> sp_runtime::DispatchResult {
		let issuance_increase_in_period = context.asset_issuance.saturating_sub(last_issuance);
		if issuance_increase_in_period > context.limit {
			let to_lock = context
				.asset_issuance
				.saturating_sub(last_issuance.saturating_add(context.limit));
			Pallet::<T>::do_lock_deposit(who, currency_id, to_lock)?;
			Pallet::<T>::do_lockdown_asset(currency_id, context.lockdown_until)?;
		}
		Ok(())
	}
}

impl<T: Config> OnDeposit<T::AccountId, T::AssetId, T::Balance> for IssuanceIncreaseFuse<T> {
	fn on_deposit(currency_id: T::AssetId, who: &T::AccountId, amount: T::Balance) -> sp_runtime::DispatchResult {
		let Some(context) = Self::get_context(currency_id) else {
			return Ok(());
		};

		Self::process_deposit_action(&context, who, currency_id, amount)
	}
}
