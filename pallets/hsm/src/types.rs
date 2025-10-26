#![allow(clippy::bad_bit_mask)]

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use codec::{Decode, Encode, MaxEncodedLen};
use evm::ExitReason;
use hydra_dx_math::hsm::CoefficientRatio;
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;
use sp_runtime::{Perbill, Permill};
use sp_std::vec::Vec;

pub type Balance = u128;

/// Type for EVM call result
pub type CallResult = (ExitReason, Vec<u8>);

/// Information about a collateral asset
#[derive(Encode, Decode, Eq, PartialEq, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CollateralInfo<AssetId> {
	/// Pool ID - asset ID where the stable asset belongs
	pub pool_id: AssetId,
	/// Purchase fee applied when buying Hollar with this asset
	pub purchase_fee: Permill,
	/// Maximum buy price coefficient - max buy price coefficient for HSM to buy back Hollar
	pub max_buy_price_coefficient: CoefficientRatio,
	/// Parameter that controls how quickly HSM can buy Hollar with this asset
	pub buyback_rate: Perbill,
	/// Fee applied when buying back Hollar
	pub buy_back_fee: Permill,
	/// Maximum amount of collateral that HSM can hold
	pub max_in_holding: Option<Balance>,
}

#[derive(Encode, Decode, Eq, PartialEq, Clone, Copy, RuntimeDebug, TypeInfo)]
#[repr(u8)]
pub enum Arbitrage {
	/// Sell HOLLAR to a pool, buy HOLLAR fro HSM
	/// Balance parameter - amount of arb amount in HOLLAR
	HollarOut(Balance) = 1, // Covers state with less HOLLAR in pool
	/// Sell HOLLAR to HSM, buy HOLLAR from a pool
	/// Balance parameter - amount of arb amount in HOLLAR
	HollarIn(Balance) = 2, // Covers state with more HOLLAR in pool.
}

impl From<Arbitrage> for (u8, Balance) {
	fn from(arb: Arbitrage) -> (u8, Balance) {
		match arb {
			Arbitrage::HollarOut(a) => (1, a),
			Arbitrage::HollarIn(a) => (2, a),
		}
	}
}

impl From<(u8, Balance)> for Arbitrage {
	fn from(value: (u8, Balance)) -> Self {
		match value.0 {
			1 => Arbitrage::HollarOut(value.1),
			2 => Arbitrage::HollarIn(value.1),
			_ => Arbitrage::HollarOut(0),
		}
	}
}
