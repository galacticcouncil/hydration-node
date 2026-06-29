use frame_support::sp_runtime::traits::Zero;
use frame_support::sp_runtime::{DispatchError, Permill, Saturating};
use sp_std::vec::Vec;

// ---------------------------------------------------------------------------
// FeeReceiver — trait for fee distribution recipients.
// Used by pallet-fee-processor to distribute trading fees.
// ---------------------------------------------------------------------------

/// A resolved fee destination: the account a receiver's slice is paid to, its
/// share, and the two flags the processor needs to route the slice correctly.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FeeDestination<AccountId> {
	/// Account that receives the slice.
	pub account: AccountId,
	/// Receiver's share of the total fee.
	pub percentage: Permill,
	/// Receiver takes its slice in the raw (unconverted) trade-fee asset
	/// instead of HDX (handled via `on_raw_fee_received`).
	pub accepts_raw: bool,
	/// When the slice is paid in HDX, hold it in the pot while `account` is
	/// below the existential deposit and flush only once the accumulated amount
	/// would lift it to/above ED. Ignored for raw receivers.
	pub hold_until_ed: bool,
}

/// Trait for fee distribution recipients.
/// Implemented by each fee receiver (staking, referrals, etc.).
///
/// Most receivers want their slice in HDX: the fee-processor converts the
/// non-HDX fee to HDX and transfers it to `destination()`. A receiver that
/// returns `true` from `accepts_raw_asset()` instead receives its slice in the
/// original (unconverted) asset and handles conversion/accounting itself
/// (used by pallet-referrals).
///
/// A raw receiver may consume LESS than the slice it is offered (e.g. an
/// unlinked trade mints no referral shares). `on_raw_fee_received` returns how
/// much it actually wants, per destination; the processor transfers only that
/// and leaves the remainder with the fee `source` — nothing is socialized.
pub trait FeeReceiver<AccountId, AssetId, Balance> {
	type Error;

	/// Account where the fee slice should be deposited.
	fn destination() -> AccountId;

	/// Percentage of total fees to receive.
	fn percentage() -> Permill;

	/// Whether this receiver accepts the raw (unconverted) trade-fee asset.
	/// When `true`, the processor offers the slice in the original asset via
	/// `on_raw_fee_received` instead of converting it to HDX.
	fn accepts_raw_asset() -> bool {
		false
	}

	/// Whether the processor should buffer this receiver's HDX slices in the pot
	/// while its account sits below the existential deposit, flushing only once
	/// the accumulated amount would lift it to/above ED. Defaults to `true` so a
	/// receiver whose pot may be uninitialized never reverts a trade with
	/// `Token::BelowMinimum`. Receivers paid in a raw asset, or whose account is
	/// always provider-backed, can override to `false` to be paid every slice
	/// immediately.
	fn hold_until_ed() -> bool {
		true
	}

	/// Returns all resolved `FeeDestination`s.
	/// Individual receiver: returns a single entry.
	/// Tuple: returns the combined list from all receivers.
	fn destinations() -> Vec<FeeDestination<AccountId>> {
		sp_std::vec![FeeDestination {
			account: Self::destination(),
			percentage: Self::percentage(),
			accepts_raw: Self::accepts_raw_asset(),
			hold_until_ed: Self::hold_until_ed(),
		}]
	}

	/// Offer a raw-asset receiver a slice of `amount` in `asset` for `trader`.
	/// Returns the `(destination, amount_used)` entries it actually wants — the
	/// processor transfers exactly `amount_used` from the fee source to each
	/// destination and leaves any unconsumed remainder with the source. Only
	/// invoked for receivers that return `true` from `accepts_raw_asset()`.
	fn on_raw_fee_received(
		_trader: AccountId,
		_asset: AssetId,
		_amount: Balance,
	) -> Result<Vec<(AccountId, Balance)>, Self::Error> {
		Ok(Vec::new())
	}
}

/// No-op implementation.
impl<AccountId: Default, AssetId, Balance> FeeReceiver<AccountId, AssetId, Balance> for () {
	type Error = DispatchError;

	fn destination() -> AccountId {
		AccountId::default()
	}

	fn percentage() -> Permill {
		Permill::zero()
	}

	fn destinations() -> Vec<FeeDestination<AccountId>> {
		Vec::new()
	}
}

/// Auto-generate tuple implementations for 1 to 6 receivers.
/// Pattern follows router's TradeExecution trait.
#[impl_trait_for_tuples::impl_for_tuples(1, 6)]
impl<
		AccountId: Clone,
		AssetId: Clone,
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
	> FeeReceiver<AccountId, AssetId, Balance> for Tuple
{
	for_tuples!( where #(Tuple: FeeReceiver<AccountId, AssetId, Balance, Error=DispatchError>)* );
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

	fn destinations() -> Vec<FeeDestination<AccountId>> {
		let mut result = Vec::new();
		for_tuples!( #( result.extend(Tuple::destinations()); )* );
		result
	}

	fn on_raw_fee_received(
		trader: AccountId,
		asset: AssetId,
		total: Balance,
	) -> Result<Vec<(AccountId, Balance)>, Self::Error> {
		let mut result = Vec::new();
		for_tuples!(
			#(
				if Tuple::accepts_raw_asset() {
					let amount = Tuple::percentage().mul_floor(total);
					if !amount.is_zero() {
						result.extend(Tuple::on_raw_fee_received(trader.clone(), asset.clone(), amount)?);
					}
				}
			)*
		);
		Ok(result)
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
