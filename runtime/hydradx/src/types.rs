use crate::*;
use frame_support::parameter_types;

use hydradx_adapters::OraclePriceProvider;

use hydradx_adapters::price::OraclePriceProviderUsingRoute;
use hydradx_traits::OraclePeriod;

parameter_types! {
	pub const LastBlockPeriod: OraclePeriod = OraclePeriod::LastBlock;
	pub const ShortPeriod: OraclePeriod = OraclePeriod::Short;
}

// Helper aliases for the OraclePriceProvider using the Router and EmaOracle
pub type LastBlockOraclePrice =
	OraclePriceProviderUsingRoute<Router, OraclePriceProvider<AssetId, EmaOracle, LRNA>, LastBlockPeriod>;
pub type ShortOraclePrice =
	OraclePriceProviderUsingRoute<Router, OraclePriceProvider<AssetId, EmaOracle, LRNA>, ShortPeriod>;
