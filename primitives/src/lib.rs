#![cfg_attr(not(feature = "std"), no_std)]

#![allow(clippy::upper_case_acronyms)]

use codec::{Decode, Encode};

use frame_support::sp_runtime::FixedU128;
use primitive_types::U256;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub mod asset;
pub mod traits;

/// An index to a block.
pub type BlockNumber = u32;

/// Type used for expressing timestamp.
pub type Moment = u64;

/// Core asset id
pub const CORE_ASSET_ID: AssetId = 0;

/// Type for storing the id of an asset.
pub type AssetId = u32;

/// Type for storing the balance of an account.
pub type Balance = u128;

/// Signed version of Balance
pub type Amount = i128;

/// Price
pub type Price = FixedU128;

/// Max fraction of pool to buy in single transaction
pub const MAX_OUT_RATIO: u128 = 3;

/// Max fraction of pool to sell in single transaction
pub const MAX_IN_RATIO: u128 = 3;

/// Trading limit
pub const MIN_TRADING_LIMIT: Balance = 1000;

/// Scaled Unsigned of Balance
pub type HighPrecisionBalance = U256;
pub type LowPrecisionBalance = u128;

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Debug, Encode, Decode, Clone, Copy, PartialEq, Eq)]
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
pub struct ExchangeIntention<AccountId, Balance, IntentionID> {
	pub who: AccountId,
	pub assets: asset::AssetPair,
	pub amount_in: Balance,
	pub amount_out: Balance,
	pub trade_limit: Balance,
	pub discount: bool,
	pub sell_or_buy: IntentionType,
	pub intention_id: IntentionID,
}

pub mod fee {
	use crate::Balance;

	#[derive(Clone, Copy, Eq, PartialEq)]
	pub struct Fee {
		pub numerator: u32,
		pub denominator: u32,
	}

	impl Default for Fee {
		fn default() -> Self {
			Fee {
				numerator: 2,
				denominator: 1000,
			} // 0.2%
		}
	}

	pub trait WithFee
	where
		Self: Sized,
	{
		fn with_fee(&self, fee: Fee) -> Option<Self>;
		fn just_fee(&self, fee: Fee) -> Option<Self>;
		fn discounted_fee(&self) -> Option<Self>;
	}

	impl WithFee for Balance {
		fn with_fee(&self, fee: Fee) -> Option<Self> {
			self.checked_mul(fee.denominator as Self - fee.numerator as Self)?
				.checked_div(fee.denominator as Self)
		}

		fn just_fee(&self, fee: Fee) -> Option<Self> {
			self.checked_mul(fee.numerator as Self)?
				.checked_div(fee.denominator as Self)
		}

		fn discounted_fee(&self) -> Option<Self> {
			let fee = Fee {
				numerator: 7,
				denominator: 10000,
			};
			self.just_fee(fee)
		}
	}
}
