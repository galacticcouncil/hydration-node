#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};

use frame_support::sp_runtime::FixedU128;
use primitive_types::U256;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub mod traits;

/// An index to a block.
pub type BlockNumber = u32;

/// Type used for expressing timestamp.
pub type Moment = u64;

/// Core asset id
pub const CORE_ASSET_ID: AssetId = 0;

/// Balance of an account.
pub type AssetId = u32;

/// Balance of an account.
pub type Balance = u128;

/// Signed version of Balance
pub type Amount = i128;

/// Price
pub type Price = FixedU128;

/// Scaled Unsigned of Balance
pub type HighPrecisionBalance = U256;
pub type LowPrecisionBalance = u128;

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
pub enum IntentionType {
	SELL,
	BUY,
}

impl Default for IntentionType {
	fn default() -> IntentionType {
		IntentionType::SELL
	}
}

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Encode, Decode, Default, Clone, PartialEq)]
pub struct ExchangeIntention<AccountId, AssetId, Balance, IntentionID> {
	pub who: AccountId,
	pub asset_sell: AssetId,
	pub asset_buy: AssetId,
	pub amount_sell: Balance,
	pub amount_buy: Balance,
	pub trade_limit: Balance,
	pub discount: bool,
	pub sell_or_buy: IntentionType,
	pub intention_id: IntentionID,
}

// FIXME: removed once HDX pallet updated to use fee::apply_fee
pub const DEFAULT_FEE_RATE: u128 = 998;

pub mod fee {
	use crate::Balance;

	pub const FEE_RATE: Balance = 998;
	pub const FEE_RATE_M: Balance = 1000;
	const FIXED_ROUND_UP: Balance = 1;

	pub const DISCOUNT_FEE_RATE: Balance = 9993;
	pub const DISCOUNT_FEE_RATE_M: Balance = 10000;

	pub fn apply_fee(amount: Balance) -> Option<Balance> {
		amount.checked_mul(FEE_RATE)?.checked_div(FEE_RATE_M)
	}

	pub fn get_fee(amount: Balance) -> Option<Balance> {
		amount.checked_mul(FEE_RATE_M - FEE_RATE)?.checked_div(FEE_RATE_M)
	}

	pub fn get_discounted_fee(amount: Balance) -> Option<Balance> {
		amount
			.checked_mul(DISCOUNT_FEE_RATE_M - DISCOUNT_FEE_RATE)?
			.checked_div(DISCOUNT_FEE_RATE_M)
	}

	// Round up
	pub fn fixed_fee(amount: Balance) -> Option<Balance> {
		amount.checked_add(FIXED_ROUND_UP)
	}
}
