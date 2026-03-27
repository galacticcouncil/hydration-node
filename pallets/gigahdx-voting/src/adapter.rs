//! GigaHdxVotingCurrency — combined GIGAHDX + HDX Currency adapter for conviction-voting.
//!
//! conviction-voting requires `Currency: ReservableCurrency + LockableCurrency + fungible::Inspect`.
//! It actually calls: `total_balance(who)`, `set_lock(...)`, `extend_lock(...)`, `remove_lock(...)`.
//! Transfer, withdraw, deposit, reserve, slash are never called.

use crate::types::VotingLockSplit;
use crate::{Config, GigaHdxVotingLock, LockSplit};
use frame_support::{
	defensive,
	pallet_prelude::Get,
	traits::{
		fungible,
		tokens::{DepositConsequence, Fortitude, Preservation, Provenance, WithdrawConsequence},
		Currency, ExistenceRequirement, LockIdentifier, LockableCurrency, ReservableCurrency, SignedImbalance, TryDrop,
		WithdrawReasons,
	},
};
use frame_system::pallet_prelude::BlockNumberFor;
use primitives::Balance;
use sp_runtime::{traits::Zero, DispatchError, DispatchResult};
use sp_std::marker::PhantomData;

/// Combined GIGAHDX + HDX currency for conviction-voting.
pub struct GigaHdxVotingCurrency<T>(PhantomData<T>);

// ---------------------------------------------------------------------------
// Minimal imbalance types (conviction-voting never constructs them)
// ---------------------------------------------------------------------------

/// Positive imbalance (stub).
pub struct PositiveImbalance(Balance);
/// Negative imbalance (stub).
pub struct NegativeImbalance(Balance);

impl Default for PositiveImbalance {
	fn default() -> Self {
		PositiveImbalance(0)
	}
}

impl Default for NegativeImbalance {
	fn default() -> Self {
		NegativeImbalance(0)
	}
}

impl Drop for PositiveImbalance {
	fn drop(&mut self) {}
}

impl Drop for NegativeImbalance {
	fn drop(&mut self) {}
}

impl TryDrop for PositiveImbalance {
	fn try_drop(self) -> Result<(), Self> {
		if self.0.is_zero() {
			Ok(())
		} else {
			Err(self)
		}
	}
}

impl TryDrop for NegativeImbalance {
	fn try_drop(self) -> Result<(), Self> {
		if self.0.is_zero() {
			Ok(())
		} else {
			Err(self)
		}
	}
}

impl frame_support::traits::tokens::imbalance::TryMerge for PositiveImbalance {
	fn try_merge(self, other: Self) -> Result<Self, (Self, Self)> {
		Ok(PositiveImbalance(self.0.saturating_add(other.0)))
	}
}

impl frame_support::traits::tokens::imbalance::TryMerge for NegativeImbalance {
	fn try_merge(self, other: Self) -> Result<Self, (Self, Self)> {
		Ok(NegativeImbalance(self.0.saturating_add(other.0)))
	}
}

impl frame_support::traits::Imbalance<Balance> for PositiveImbalance {
	type Opposite = NegativeImbalance;

	fn zero() -> Self {
		PositiveImbalance(0)
	}

	fn drop_zero(self) -> Result<(), Self> {
		if self.0.is_zero() {
			Ok(())
		} else {
			Err(self)
		}
	}

	fn split(self, amount: Balance) -> (Self, Self) {
		let first = self.0.min(amount);
		let second = self.0.saturating_sub(first);
		(PositiveImbalance(first), PositiveImbalance(second))
	}

	fn extract(&mut self, amount: Balance) -> Self {
		let taken = self.0.min(amount);
		self.0 = self.0.saturating_sub(taken);
		PositiveImbalance(taken)
	}

	fn merge(self, other: Self) -> Self {
		PositiveImbalance(self.0.saturating_add(other.0))
	}

	fn subsume(&mut self, other: Self) {
		self.0 = self.0.saturating_add(other.0);
	}

	fn offset(self, other: Self::Opposite) -> frame_support::traits::SameOrOther<Self, Self::Opposite> {
		use frame_support::traits::SameOrOther;
		let (a, b) = (self.0, other.0);
		if a >= b {
			SameOrOther::Same(PositiveImbalance(a - b))
		} else {
			SameOrOther::Other(NegativeImbalance(b - a))
		}
	}

	fn peek(&self) -> Balance {
		self.0
	}
}

impl frame_support::traits::Imbalance<Balance> for NegativeImbalance {
	type Opposite = PositiveImbalance;

	fn zero() -> Self {
		NegativeImbalance(0)
	}

	fn drop_zero(self) -> Result<(), Self> {
		if self.0.is_zero() {
			Ok(())
		} else {
			Err(self)
		}
	}

	fn split(self, amount: Balance) -> (Self, Self) {
		let first = self.0.min(amount);
		let second = self.0.saturating_sub(first);
		(NegativeImbalance(first), NegativeImbalance(second))
	}

	fn extract(&mut self, amount: Balance) -> Self {
		let taken = self.0.min(amount);
		self.0 = self.0.saturating_sub(taken);
		NegativeImbalance(taken)
	}

	fn merge(self, other: Self) -> Self {
		NegativeImbalance(self.0.saturating_add(other.0))
	}

	fn subsume(&mut self, other: Self) {
		self.0 = self.0.saturating_add(other.0);
	}

	fn offset(self, other: Self::Opposite) -> frame_support::traits::SameOrOther<Self, Self::Opposite> {
		use frame_support::traits::SameOrOther;
		let (a, b) = (self.0, other.0);
		if a >= b {
			SameOrOther::Same(NegativeImbalance(a - b))
		} else {
			SameOrOther::Other(PositiveImbalance(b - a))
		}
	}

	fn peek(&self) -> Balance {
		self.0
	}
}

// ---------------------------------------------------------------------------
// Helper to get GIGAHDX balance
// ---------------------------------------------------------------------------

fn gigahdx_balance<T: Config>(who: &T::AccountId) -> Balance {
	<T::Currency as frame_support::traits::fungibles::Inspect<T::AccountId>>::balance(
		<T as pallet_gigahdx::Config>::GigaHdxAssetId::get(),
		who,
	)
}

fn hdx_balance<T: Config>(who: &T::AccountId) -> Balance {
	<T::NativeCurrency as fungible::Inspect<T::AccountId>>::total_balance(who)
}

// ---------------------------------------------------------------------------
// fungible::Inspect — conviction-voting calls total_balance(who)
// ---------------------------------------------------------------------------

impl<T: Config> fungible::Inspect<T::AccountId> for GigaHdxVotingCurrency<T> {
	type Balance = Balance;

	fn total_issuance() -> Balance {
		let gigahdx_issuance = <T::Currency as frame_support::traits::fungibles::Inspect<T::AccountId>>::total_issuance(
			<T as pallet_gigahdx::Config>::GigaHdxAssetId::get(),
		);
		let hdx_issuance = <T::NativeCurrency as fungible::Inspect<T::AccountId>>::total_issuance();
		gigahdx_issuance.saturating_add(hdx_issuance)
	}

	fn minimum_balance() -> Balance {
		Zero::zero()
	}

	fn total_balance(who: &T::AccountId) -> Balance {
		gigahdx_balance::<T>(who).saturating_add(hdx_balance::<T>(who))
	}

	fn balance(who: &T::AccountId) -> Balance {
		<Self as fungible::Inspect<T::AccountId>>::total_balance(who)
	}

	fn reducible_balance(who: &T::AccountId, _preservation: Preservation, _force: Fortitude) -> Balance {
		<Self as fungible::Inspect<T::AccountId>>::total_balance(who)
	}

	fn can_deposit(_who: &T::AccountId, _amount: Balance, _provenance: Provenance) -> DepositConsequence {
		DepositConsequence::Success
	}

	fn can_withdraw(who: &T::AccountId, amount: Balance) -> WithdrawConsequence<Balance> {
		if <Self as fungible::Inspect<T::AccountId>>::total_balance(who) >= amount {
			WithdrawConsequence::Success
		} else {
			WithdrawConsequence::BalanceLow
		}
	}
}

// ---------------------------------------------------------------------------
// Currency (legacy) — conviction-voting calls total_balance, free_balance
// ---------------------------------------------------------------------------

impl<T: Config> Currency<T::AccountId> for GigaHdxVotingCurrency<T> {
	type Balance = Balance;
	type PositiveImbalance = PositiveImbalance;
	type NegativeImbalance = NegativeImbalance;

	fn total_balance(who: &T::AccountId) -> Balance {
		<Self as fungible::Inspect<T::AccountId>>::total_balance(who)
	}

	fn can_slash(_who: &T::AccountId, _value: Balance) -> bool {
		false
	}

	fn total_issuance() -> Balance {
		<Self as fungible::Inspect<T::AccountId>>::total_issuance()
	}

	fn minimum_balance() -> Balance {
		Zero::zero()
	}

	fn burn(_amount: Balance) -> Self::PositiveImbalance {
		defensive!("GigaHdxVotingCurrency::burn should never be called");
		PositiveImbalance(0)
	}

	fn issue(_amount: Balance) -> Self::NegativeImbalance {
		defensive!("GigaHdxVotingCurrency::issue should never be called");
		NegativeImbalance(0)
	}

	fn free_balance(who: &T::AccountId) -> Balance {
		Self::total_balance(who)
	}

	fn ensure_can_withdraw(
		_who: &T::AccountId,
		_amount: Balance,
		_reasons: WithdrawReasons,
		_new_balance: Balance,
	) -> DispatchResult {
		Ok(())
	}

	fn transfer(
		_source: &T::AccountId,
		_dest: &T::AccountId,
		_value: Balance,
		_existence_requirement: ExistenceRequirement,
	) -> DispatchResult {
		defensive!("GigaHdxVotingCurrency::transfer should never be called");
		Ok(())
	}

	fn slash(_who: &T::AccountId, _value: Balance) -> (Self::NegativeImbalance, Balance) {
		defensive!("GigaHdxVotingCurrency::slash should never be called");
		(NegativeImbalance(0), 0)
	}

	fn deposit_into_existing(_who: &T::AccountId, _value: Balance) -> Result<Self::PositiveImbalance, DispatchError> {
		defensive!("GigaHdxVotingCurrency::deposit_into_existing should never be called");
		Ok(PositiveImbalance(0))
	}

	fn deposit_creating(_who: &T::AccountId, _value: Balance) -> Self::PositiveImbalance {
		defensive!("GigaHdxVotingCurrency::deposit_creating should never be called");
		PositiveImbalance(0)
	}

	fn withdraw(
		_who: &T::AccountId,
		_value: Balance,
		_reasons: WithdrawReasons,
		_liveness: ExistenceRequirement,
	) -> Result<Self::NegativeImbalance, DispatchError> {
		defensive!("GigaHdxVotingCurrency::withdraw should never be called");
		Ok(NegativeImbalance(0))
	}

	fn make_free_balance_be(
		_who: &T::AccountId,
		_balance: Balance,
	) -> SignedImbalance<Balance, Self::PositiveImbalance> {
		defensive!("GigaHdxVotingCurrency::make_free_balance_be should never be called");
		SignedImbalance::Positive(PositiveImbalance(0))
	}
}

// ---------------------------------------------------------------------------
// LockableCurrency — the core lock split logic
// ---------------------------------------------------------------------------

impl<T: Config> LockableCurrency<T::AccountId> for GigaHdxVotingCurrency<T> {
	type Moment = BlockNumberFor<T>;
	type MaxLocks = ();

	fn set_lock(id: LockIdentifier, who: &T::AccountId, amount: Balance, _reasons: WithdrawReasons) {
		Self::apply_lock_split(id, who, amount);
	}

	fn extend_lock(id: LockIdentifier, who: &T::AccountId, amount: Balance, _reasons: WithdrawReasons) {
		let current = LockSplit::<T>::get(who);
		let current_total = current.gigahdx_amount.saturating_add(current.hdx_amount);
		if amount >= current_total {
			Self::apply_lock_split(id, who, amount);
		}
	}

	fn remove_lock(id: LockIdentifier, who: &T::AccountId) {
		GigaHdxVotingLock::<T>::remove(who);
		T::NativeCurrency::remove_lock(id, who);
		LockSplit::<T>::remove(who);
	}
}

impl<T: Config> GigaHdxVotingCurrency<T> {
	fn apply_lock_split(id: LockIdentifier, who: &T::AccountId, amount: Balance) {
		let gigahdx_bal = gigahdx_balance::<T>(who);

		let gigahdx_lock = amount.min(gigahdx_bal);
		let hdx_lock = amount.saturating_sub(gigahdx_lock);

		GigaHdxVotingLock::<T>::insert(who, gigahdx_lock);

		LockSplit::<T>::insert(
			who,
			VotingLockSplit {
				gigahdx_amount: gigahdx_lock,
				hdx_amount: hdx_lock,
			},
		);

		if hdx_lock > Zero::zero() {
			T::NativeCurrency::set_lock(id, who, hdx_lock, WithdrawReasons::all());
		} else {
			T::NativeCurrency::remove_lock(id, who);
		}
	}
}

// ---------------------------------------------------------------------------
// ReservableCurrency — stubs (never called by conviction-voting)
// ---------------------------------------------------------------------------

impl<T: Config> ReservableCurrency<T::AccountId> for GigaHdxVotingCurrency<T> {
	fn can_reserve(_who: &T::AccountId, _value: Balance) -> bool {
		true
	}

	fn slash_reserved(_who: &T::AccountId, _value: Balance) -> (Self::NegativeImbalance, Balance) {
		defensive!("GigaHdxVotingCurrency::slash_reserved should never be called");
		(NegativeImbalance(0), 0)
	}

	fn reserved_balance(_who: &T::AccountId) -> Balance {
		Zero::zero()
	}

	fn reserve(_who: &T::AccountId, _value: Balance) -> DispatchResult {
		defensive!("GigaHdxVotingCurrency::reserve should never be called");
		Ok(())
	}

	fn unreserve(_who: &T::AccountId, _value: Balance) -> Balance {
		defensive!("GigaHdxVotingCurrency::unreserve should never be called");
		Zero::zero()
	}

	fn repatriate_reserved(
		_slashed: &T::AccountId,
		_beneficiary: &T::AccountId,
		_value: Balance,
		_status: frame_support::traits::BalanceStatus,
	) -> Result<Balance, DispatchError> {
		defensive!("GigaHdxVotingCurrency::repatriate_reserved should never be called");
		Ok(Zero::zero())
	}
}
