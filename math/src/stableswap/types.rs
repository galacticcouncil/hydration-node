use crate::types::Balance;
use num_traits::Zero;

#[derive(Debug, Clone, Copy)]
pub struct AssetReserve {
	pub amount: Balance,
	pub decimals: u8,
}

impl AssetReserve {
	pub fn new(amount: Balance, decimals: u8) -> Self {
		Self { amount, decimals }
	}

	pub fn is_zero(&self) -> bool {
		self.amount == Balance::zero()
	}
}

impl From<AssetReserve> for u128 {
	fn from(value: AssetReserve) -> Self {
		value.amount
	}
}
impl From<&AssetReserve> for u128 {
	fn from(value: &AssetReserve) -> Self {
		value.amount
	}
}
