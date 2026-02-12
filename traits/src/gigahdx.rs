use frame_support::sp_runtime::traits::Zero;
use frame_support::sp_runtime::{DispatchError, Permill, Saturating};
use sp_std::vec::Vec;

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
