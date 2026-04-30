use codec::{Decode, Encode};
use frame_support::dispatch::DispatchResult;
use frame_support::sp_runtime::traits::Zero;
use frame_support::sp_runtime::{DispatchError, Permill, RuntimeDebug, Saturating};
use scale_info::TypeInfo;
use sp_std::vec::Vec;

// ---------------------------------------------------------------------------
// GigaHdxHooks — called by pallet-gigahdx during stake/unstake lifecycle.
// Implemented by pallet-gigahdx-voting.
// ---------------------------------------------------------------------------

/// Hooks called by pallet-gigahdx during stake/unstake lifecycle.
pub trait GigaHdxHooks<AccountId, Balance, BlockNumber> {
	/// Called after a successful giga_stake.
	fn on_stake(who: &AccountId, hdx_amount: Balance, gigahdx_received: Balance) -> DispatchResult;

	/// Called during giga_unstake after can_unstake check passes.
	/// Handles force removal of votes from finished referenda and records rewards.
	fn on_unstake(who: &AccountId, gigahdx_amount: Balance) -> DispatchResult;

	/// Check if the user can unstake `amount` of GIGAHDX.
	///
	/// Returns `false` only if `amount` would dip the user's GIGAHDX balance
	/// below the lock committed by votes on **ongoing** referenda. Votes on
	/// finished referenda are force-removable by `on_unstake` and don't gate
	/// here. If the user has no ongoing votes, any `amount` ≤ balance is
	/// permitted (downstream AAVE.withdraw enforces the balance check).
	fn can_unstake(who: &AccountId, amount: Balance) -> bool;

	/// Get the additional lock period required due to voting locks.
	/// Returns the maximum remaining lock duration across all votes.
	/// Called BEFORE on_unstake to capture lock periods before votes are removed.
	fn additional_unstake_lock(who: &AccountId) -> BlockNumber;

	/// Called AFTER `giga_unstake` has reduced the user's GIGAHDX balance via MM withdraw.
	///
	/// Re-applies any existing voting-lock split against the user's new balance,
	/// capping the GIGAHDX-side tracker and spilling uncovered commitment onto
	/// a hard HDX lock. No-op when the user has no voting lock.
	fn on_post_unstake(who: &AccountId) -> DispatchResult;
}

/// No-op implementation — used when no voting pallet is wired (e.g. tests).
impl<AccountId, Balance, BlockNumber: frame_support::sp_runtime::traits::Zero>
	GigaHdxHooks<AccountId, Balance, BlockNumber> for ()
{
	fn on_stake(_who: &AccountId, _hdx_amount: Balance, _gigahdx_received: Balance) -> DispatchResult {
		Ok(())
	}
	fn on_unstake(_who: &AccountId, _gigahdx_amount: Balance) -> DispatchResult {
		Ok(())
	}
	fn can_unstake(_who: &AccountId, _amount: Balance) -> bool {
		true
	}
	fn additional_unstake_lock(_who: &AccountId) -> BlockNumber {
		frame_support::sp_runtime::traits::Zero::zero()
	}
	fn on_post_unstake(_who: &AccountId) -> DispatchResult {
		Ok(())
	}
}

// ---------------------------------------------------------------------------
// MoneyMarketOperations — supply/withdraw through a Money Market (e.g. AAVE).
// ---------------------------------------------------------------------------

/// Money Market supply/withdraw operations.
/// Implemented by a runtime adapter wrapping AAVE EVM calls.
pub trait MoneyMarketOperations<AccountId, AssetId, Balance> {
	/// Supply underlying asset to Money Market, receive aToken.
	/// Returns the amount of aToken received.
	fn supply(who: &AccountId, underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError>;

	/// Withdraw from Money Market, burn aToken, receive underlying.
	/// Returns the amount of underlying received.
	fn withdraw(who: &AccountId, underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError>;

	/// Return the user's current aToken (GIGAHDX) balance in the Money Market.
	fn balance_of(who: &AccountId) -> Balance;
}

/// No-op implementation — supply/withdraw are identity (amount in == amount out).
impl<AccountId, AssetId, Balance: Zero> MoneyMarketOperations<AccountId, AssetId, Balance> for () {
	fn supply(_who: &AccountId, _underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		Ok(amount)
	}
	fn withdraw(_who: &AccountId, _underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		Ok(amount)
	}
	fn balance_of(_who: &AccountId) -> Balance {
		Zero::zero()
	}
}

// ---------------------------------------------------------------------------
// Referendum state queries — used by pallet-gigahdx-voting for reward logic.
// ---------------------------------------------------------------------------

/// Referendum outcome for reward calculations.
#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub enum ReferendumOutcome {
	/// Referendum is still ongoing (voting active).
	Ongoing,
	/// Referendum was approved (passed).
	Approved,
	/// Referendum was rejected (failed).
	Rejected,
	/// Referendum was cancelled / timed-out / killed (no rewards).
	Cancelled,
}

/// Query referendum state / outcome.
/// Extends the simpler `GetReferendumState` from pallet-staking with full outcome info.
pub trait GetReferendumOutcome<Index> {
	type BlockNumber;

	/// Check if referendum is finished (not ongoing).
	fn is_referendum_finished(index: Index) -> bool;

	/// Get the full referendum outcome.
	fn referendum_outcome(index: Index) -> ReferendumOutcome;

	/// Completion block for finished referenda; `None` if ongoing or unknown.
	fn end_block(index: Index) -> Option<Self::BlockNumber>;
}

/// Query track ID for a given referendum index.
pub trait GetTrackId<Index> {
	type TrackId;

	/// Get the track ID for a given referendum index.
	/// Returns `None` if the referendum doesn't exist.
	fn track_id(index: Index) -> Option<Self::TrackId>;
}

// ---------------------------------------------------------------------------
// TrackRewardConfig — per-track reward percentage configuration.
// ---------------------------------------------------------------------------

/// Per-track reward percentage configuration.
/// Implemented in the runtime to set different reward percentages per governance track.
pub trait TrackRewardConfig {
	/// Get the reward percentage for a specific track.
	fn reward_percentage(track_id: u16) -> Permill;
}

// ---------------------------------------------------------------------------
// FeeReceiver — trait for fee distribution recipients.
// Used by pallet-fee-processor to distribute trading fees.
// ---------------------------------------------------------------------------

/// Trait for fee distribution recipients.
/// Implemented by each fee receiver (staking, gigahdx, referrals, etc.).
pub trait FeeReceiver<AccountId, Balance> {
	type Error;

	/// Account where HDX should be deposited.
	fn destination() -> AccountId;

	/// Percentage of total fees to receive.
	fn percentage() -> Permill;

	/// Returns all (destination, percentage) pairs for distribution.
	/// Individual receiver: returns `vec![(destination(), percentage())]`.
	/// Tuple: returns combined list from all receivers.
	fn destinations() -> Vec<(AccountId, Permill)> {
		sp_std::vec![(Self::destination(), Self::percentage())]
	}

	/// Optimistic pre-deposit callback with trader context.
	/// Called BEFORE actual HDX transfer/conversion. The amount is based on
	/// spot price and may differ from the final transfer amount.
	fn on_pre_fee_deposit(trader: AccountId, amount: Balance) -> Result<(), Self::Error>;

	/// Post-deposit callback after HDX has been distributed to pots.
	/// Called after `distribute_to_pots` completes. No trader context needed.
	fn on_fee_received(amount: Balance) -> Result<(), Self::Error>;
}

// ---------------------------------------------------------------------------
// PrepareForLiquidation — clear GIGAHDX voting locks before liquidation.
// Used by pallet-liquidation when liquidating GIGAHDX collateral.
// ---------------------------------------------------------------------------

/// Clear GIGAHDX voting locks before liquidation.
/// Implemented by pallet-gigahdx-voting.
pub trait PrepareForLiquidation<AccountId> {
	fn prepare_for_liquidation(who: &AccountId) -> DispatchResult;
}

/// No-op implementation (for runtimes without GIGAHDX voting).
impl<AccountId> PrepareForLiquidation<AccountId> for () {
	fn prepare_for_liquidation(_who: &AccountId) -> DispatchResult {
		Ok(())
	}
}

// ---------------------------------------------------------------------------
// ForceRemoveVote — force-remove a user's vote from conviction-voting.
// Used by pallet-gigahdx-voting during unstake and liquidation.
// ---------------------------------------------------------------------------

/// Force-remove a vote from conviction-voting on behalf of a user.
/// Implemented in the runtime as a wrapper around `pallet_conviction_voting::Pallet::remove_vote`.
pub trait ForceRemoveVote<AccountId> {
	fn remove_vote(who: &AccountId, class: Option<u16>, index: u32) -> DispatchResult;
}

impl<AccountId> ForceRemoveVote<AccountId> for () {
	fn remove_vote(_who: &AccountId, _class: Option<u16>, _index: u32) -> DispatchResult {
		Ok(())
	}
}

/// No-op implementation.
impl<AccountId: Default, Balance> FeeReceiver<AccountId, Balance> for () {
	type Error = DispatchError;

	fn destination() -> AccountId {
		AccountId::default()
	}

	fn percentage() -> Permill {
		Permill::zero()
	}

	fn destinations() -> Vec<(AccountId, Permill)> {
		Vec::new()
	}

	fn on_pre_fee_deposit(_trader: AccountId, _amount: Balance) -> Result<(), Self::Error> {
		Ok(())
	}

	fn on_fee_received(_amount: Balance) -> Result<(), Self::Error> {
		Ok(())
	}
}

/// Auto-generate tuple implementations for 1 to 6 receivers.
/// Pattern follows router's TradeExecution trait.
#[impl_trait_for_tuples::impl_for_tuples(1, 6)]
impl<
		AccountId: Clone,
		Balance: Copy
			+ Zero
			+ Saturating
			+ sp_arithmetic::traits::Unsigned
			+ core::ops::Rem<Balance, Output = Balance>
			+ core::ops::Div<Balance, Output = Balance>
			+ core::ops::Mul<Balance, Output = Balance>
			+ core::ops::Add<Balance, Output = Balance>
			+ sp_arithmetic::traits::UniqueSaturatedInto<u128>
			+ sp_arithmetic::traits::UniqueSaturatedInto<u32>
			+ From<u32>,
	> FeeReceiver<AccountId, Balance> for Tuple
{
	for_tuples!( where #(Tuple: FeeReceiver<AccountId, Balance, Error=DispatchError>)* );
	type Error = DispatchError;

	fn destination() -> AccountId {
		// Not meaningful for tuple — use destinations() instead.
		panic!("destination() should not be called on tuple")
	}

	fn percentage() -> Permill {
		let mut total = Permill::zero();
		for_tuples!( #( total = total.saturating_add(Tuple::percentage()); )* );
		total
	}

	fn destinations() -> Vec<(AccountId, Permill)> {
		let mut result = Vec::new();
		for_tuples!( #( result.extend(Tuple::destinations()); )* );
		result
	}

	fn on_pre_fee_deposit(trader: AccountId, total: Balance) -> Result<(), Self::Error> {
		for_tuples!(
			#(
				let amount = Tuple::percentage().mul_floor(total);
				if !amount.is_zero() {
					Tuple::on_pre_fee_deposit(trader.clone(), amount)?;
				}
			)*
		);
		Ok(())
	}

	fn on_fee_received(total: Balance) -> Result<(), Self::Error> {
		for_tuples!(
			#(
				let amount = Tuple::percentage().mul_floor(total);
				if !amount.is_zero() {
					Tuple::on_fee_received(amount)?;
				}
			)*
		);
		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Convert — trait for converting assets via trading (e.g. Omnipool sell).
// Originally from pallet-referrals, now shared.
// ---------------------------------------------------------------------------

/// Trait for converting assets via trading (e.g. Omnipool sell).
pub trait Convert<AccountId, AssetId, Balance> {
	type Error;

	fn convert(who: AccountId, asset_from: AssetId, asset_to: AssetId, amount: Balance)
		-> Result<Balance, Self::Error>;
}
