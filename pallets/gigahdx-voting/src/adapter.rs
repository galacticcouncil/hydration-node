//! GigaHdxVotingCurrency — combined GIGAHDX + HDX Currency adapter for conviction-voting.
//!
//! conviction-voting requires `Currency: ReservableCurrency + LockableCurrency + fungible::Inspect`.
//! It actually calls: `total_balance(who)`, `set_lock(...)`, `extend_lock(...)`, `remove_lock(...)`.
//! Transfer, withdraw, deposit, reserve, slash are never called.

use crate::types::VotingLockSplit;
use crate::{Config, DelegationLockSplit, GigaHdxVotes, GigaHdxVotingLock, PriorLockSplit, UnstakeSpillover};
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

	fn set_lock(_id: LockIdentifier, who: &T::AccountId, amount: Balance, _reasons: WithdrawReasons) {
		// Used by `update_lock` after `unlock`. If `amount` exceeds what active
		// votes + priors can explain, the difference is from a delegation prior
		// upstream is keeping alive — snapshot it. If `amount == 0`, drop the
		// delegation snapshot entirely.
		if amount.is_zero() {
			DelegationLockSplit::<T>::remove(who);
		} else {
			Self::ensure_delegation_snapshot_covers(who, amount);
		}
		Self::recompute_lock_split(who);
	}

	fn extend_lock(_id: LockIdentifier, who: &T::AccountId, amount: Balance, _reasons: WithdrawReasons) {
		// For votes: `on_before_vote` has already populated `GigaHdxVotes` so
		// the recompute picks up the new contribution.
		// For delegations: there's no hook, so conviction-voting calls us
		// directly with the delegated balance — snapshot a per-side split if
		// the active votes + priors don't already cover this amount.
		Self::ensure_delegation_snapshot_covers(who, amount);
		Self::recompute_lock_split(who);
	}

	fn remove_lock(id: LockIdentifier, who: &T::AccountId) {
		GigaHdxVotingLock::<T>::remove(who);
		let _ = PriorLockSplit::<T>::clear_prefix(who, u32::MAX, None);
		DelegationLockSplit::<T>::remove(who);
		UnstakeSpillover::<T>::remove(who);
		T::NativeCurrency::remove_lock(id, who);
	}
}

impl<T: Config> GigaHdxVotingCurrency<T> {
	/// Per-side max-aggregate over (a) every active `GigaHdxVotes` entry's stored
	/// per-vote split, and (b) every still-running `PriorLockSplit` prior — same
	/// shape as upstream's `voting.locked_balance() = max(votes, prior)`.
	///
	/// Writes the resulting GIGAHDX-side cap into `GigaHdxVotingLock` (read by the
	/// 0x0806 EVM lock-manager precompile) and the HDX-side cap into the standard
	/// `pallet_balances::Locks` entry under id `pyconvot`.
	pub fn recompute_lock_split(who: &T::AccountId) {
		const CONVICTION_VOTING_LOCK_ID: LockIdentifier = *b"pyconvot";
		let now = frame_system::Pallet::<T>::block_number();
		let mut g_max: Balance = 0;
		let mut h_max: Balance = 0;

		// Active votes — every vote's stored (gigahdx_lock, hdx_lock) snapshot.
		for (_ref, v) in GigaHdxVotes::<T>::iter_prefix(who) {
			if v.gigahdx_lock > g_max {
				g_max = v.gigahdx_lock;
			}
			if v.hdx_lock > h_max {
				h_max = v.hdx_lock;
			}
		}

		// Delegation snapshot (if any) — fallback for conviction-voting paths
		// (delegate / undelegate prior) that don't go through our vote hooks.
		let delegation = DelegationLockSplit::<T>::get(who);
		if delegation.gigahdx_amount > g_max {
			g_max = delegation.gigahdx_amount;
		}
		if delegation.hdx_amount > h_max {
			h_max = delegation.hdx_amount;
		}

		// Priors — rejig expired ones, accumulate live ones into per-side max.
		let mut expired: sp_std::vec::Vec<u16> = sp_std::vec::Vec::new();
		for (class, mut p) in PriorLockSplit::<T>::iter_prefix(who) {
			p.rejig(now);
			if !p.is_active() {
				expired.push(class);
			} else {
				if p.gigahdx > g_max {
					g_max = p.gigahdx;
				}
				if p.hdx > h_max {
					h_max = p.hdx;
				}
			}
		}
		for class in expired {
			PriorLockSplit::<T>::remove(who, class);
		}

		// Unstake spillover — H-side residue from `giga_unstake` (commitment
		// that no longer fits the user's GIGAHDX balance).
		let spillover = UnstakeSpillover::<T>::get(who);
		if spillover > h_max {
			h_max = spillover;
		}

		// If there's nothing else holding the prior alive, the spillover should
		// expire too. Use upstream's `pyconvot` lock as the canonical "any
		// commitment still alive" signal: when both votes and priors clear, the
		// spillover has no commitment left to back. We approximate this by
		// dropping spillover whenever the per-side max from votes+priors is 0.
		// (Active votes/priors keep the spillover alive via their own H-side
		// contributions; the spillover only matters when those have aged out.)
		if g_max == 0
			&& GigaHdxVotes::<T>::iter_prefix(who).next().is_none()
			&& PriorLockSplit::<T>::iter_prefix(who).next().is_none()
			&& DelegationLockSplit::<T>::get(who) == VotingLockSplit::default()
		{
			UnstakeSpillover::<T>::remove(who);
			h_max = 0;
		}

		// GIGAHDX side — write the cap consumed by the EVM lock-manager precompile.
		if g_max > Zero::zero() {
			GigaHdxVotingLock::<T>::insert(who, g_max);
		} else {
			GigaHdxVotingLock::<T>::remove(who);
		}

		// HDX side — standard balances lock.
		if h_max > Zero::zero() {
			T::NativeCurrency::set_lock(CONVICTION_VOTING_LOCK_ID, who, h_max, WithdrawReasons::all());
		} else {
			T::NativeCurrency::remove_lock(CONVICTION_VOTING_LOCK_ID, who);
		}
	}

	/// If `amount` exceeds what active `GigaHdxVotes` entries + `PriorLockSplit`
	/// already account for, the extra commitment must be from a path that
	/// bypasses our vote hooks (delegate / delegation prior). Snapshot a
	/// GIGAHDX-first split for it and max-aggregate into `DelegationLockSplit`.
	fn ensure_delegation_snapshot_covers(who: &T::AccountId, amount: Balance) {
		if amount.is_zero() {
			return;
		}
		let mut g_votes_max: Balance = 0;
		let mut h_votes_max: Balance = 0;
		for (_ref, v) in GigaHdxVotes::<T>::iter_prefix(who) {
			if v.gigahdx_lock > g_votes_max {
				g_votes_max = v.gigahdx_lock;
			}
			if v.hdx_lock > h_votes_max {
				h_votes_max = v.hdx_lock;
			}
		}
		let now = frame_system::Pallet::<T>::block_number();
		for (_class, mut p) in PriorLockSplit::<T>::iter_prefix(who) {
			p.rejig(now);
			if p.is_active() {
				if p.gigahdx > g_votes_max {
					g_votes_max = p.gigahdx;
				}
				if p.hdx > h_votes_max {
					h_votes_max = p.hdx;
				}
			}
		}

		let votes_total = g_votes_max.saturating_add(h_votes_max);
		if amount <= votes_total {
			// votes + priors already cover the amount; no delegation snapshot needed
			// beyond what's already there.
			return;
		}

		// Snapshot the delegated commitment GIGAHDX-first against current
		// balance. Uses current GIGAHDX balance — same trade-off upstream's
		// `prior` makes (it captures balance at delegation time).
		let gigahdx_bal = gigahdx_balance::<T>(who);
		let g_snap = amount.min(gigahdx_bal);
		let h_snap = amount.saturating_sub(g_snap);

		DelegationLockSplit::<T>::mutate(who, |existing| {
			if g_snap > existing.gigahdx_amount {
				existing.gigahdx_amount = g_snap;
			}
			if h_snap > existing.hdx_amount {
				existing.hdx_amount = h_snap;
			}
		});
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
