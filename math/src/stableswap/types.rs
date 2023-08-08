use crate::types::Balance;

#[derive(Debug, Clone, Copy)]
pub struct AssetReserve {
	pub amount: Balance,
	pub decimals: u8,
}

impl AssetReserve {
	pub fn new(amount: Balance, decimals: u8) -> Self {
		Self { amount, decimals }
	}
}

pub(crate) fn target_precision(_reserves: &[AssetReserve]) -> u8{
	18u8
}
