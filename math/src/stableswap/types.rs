use crate::ratio::Ratio;
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

#[derive(Clone, Copy, Debug)]
pub(crate) struct PegDelta {
	pub(crate) delta: Ratio,
	pub(crate) neg: bool,
	pub(crate) block_diff: u128,
}

impl From<(Ratio, bool, u128)> for PegDelta {
	fn from(value: (Ratio, bool, u128)) -> Self {
		Self {
			delta: value.0,
			neg: value.1,
			block_diff: value.2,
		}
	}
}
