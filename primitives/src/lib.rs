#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::upper_case_acronyms)]

use codec::{Decode, Encode};

use frame_support::sp_runtime::FixedU128;
use primitive_types::U256;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
	generic,
	traits::{BlakeTwo256, IdentifyAccount, Verify},
	MultiSignature,
};

pub mod asset;
pub mod constants;
pub mod traits;

/// Opaque, encoded, unchecked extrinsic.
pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

/// The type for looking up accounts. We don't expect more than 4 billion of them, but you
/// never know...
pub type AccountIndex = u32;

/// Header type.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// Index of a transaction in the chain.
pub type Index = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// Digest item type.
pub type DigestItem = generic::DigestItem<Hash>;

/// Type used for expressing timestamp.
pub type Moment = u64;

/// Type for storing the id of an asset.
pub type AssetId = u32;

/// Type for storing the balance of an account.
pub type Balance = u128;

/// Signed version of Balance
pub type Amount = i128;

/// Price
pub type Price = FixedU128;

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
