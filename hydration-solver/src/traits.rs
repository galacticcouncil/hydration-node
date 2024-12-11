use crate::types::*;

pub trait OmnipoolInfo<AssetId> {
	fn assets(filter: Option<Vec<AssetId>>) -> Vec<OmnipoolAssetInfo<AssetId>>;
}

#[derive(Debug, serde::Deserialize)]
pub struct TempOmnipoolAssetInfo<AssetId> {
	pub asset_id: AssetId,
	pub reserve: Balance,
	pub hub_reserve: Balance,
	pub decimals: u8,
	pub fee: u32,
	pub hub_fee: u32,
}

#[derive(Debug, serde::Deserialize)]
pub struct OmnipoolAssetInfo<AssetId> {
	pub asset_id: AssetId,
	pub reserve: Balance,
	pub hub_reserve: Balance,
	pub decimals: u8,
	pub fee: (u32, u32),
	pub hub_fee: (u32, u32),
}

//TODO: this should not be aware of any f64 conversions! job for solver only
impl<AssetId> OmnipoolAssetInfo<AssetId> {
	pub fn reserve_as_f64(&self) -> f64 {
		self.reserve as f64 / 10u128.pow(self.decimals as u32) as f64
	}

	pub fn hub_reserve_as_f64(&self) -> f64 {
		self.hub_reserve as f64 / 10u128.pow(12u32) as f64
	}

	pub fn fee_as_f64(&self) -> f64 {
		self.fee.0 as f64 / self.fee.1 as f64
	}

	pub fn hub_fee_as_f64(&self) -> f64 {
		self.hub_fee.0 as f64 / self.hub_fee.1 as f64
	}
	#[cfg(test)]
	pub fn reserve_no_decimals(&self) -> Balance {
		self.reserve / 10u128.pow(self.decimals as u32)
	}
	#[cfg(test)]
	pub fn hub_reserve_no_decimals(&self) -> Balance {
		self.hub_reserve / 10u128.pow(12u32)
	}
}
