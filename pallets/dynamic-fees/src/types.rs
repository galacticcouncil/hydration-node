use frame_support::pallet_prelude::*;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::FixedU128;

use hydra_dx_math::dynamic_fees::types::FeeParams as MathFeeParams;

use scale_info::TypeInfo;

/// Parameters for dynamic fee calculation
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct FeeParams<Fee> {
	pub min_fee: Fee,
	pub max_fee: Fee,
	pub decay: FixedU128,
	pub amplification: FixedU128,
}

/// Fee entry stored in the pallet storage
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct FeeEntry<Fee, Block> {
	pub asset_fee: Fee,
	pub protocol_fee: Fee,
	/// Block number when this entry was last updated
	pub timestamp: Block,
}

/// Asset fee configuration
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AssetFeeConfig<Fee> {
	/// Fixed fee that doesn't change based on oracle data
	Fixed { asset_fee: Fee, protocol_fee: Fee },
	/// Dynamic fee that uses oracle data and custom parameters
	Dynamic {
		asset_fee_params: FeeParams<Fee>,
		protocol_fee_params: FeeParams<Fee>,
	},
}

impl<Fee> From<FeeParams<Fee>> for MathFeeParams<Fee> {
	fn from(value: FeeParams<Fee>) -> Self {
		MathFeeParams {
			min_fee: value.min_fee,
			max_fee: value.max_fee,
			decay: value.decay,
			amplification: value.amplification,
		}
	}
}
