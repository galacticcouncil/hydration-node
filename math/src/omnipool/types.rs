use crate::omnipool::types::BalanceUpdate::{Decrease, Increase};
use num_traits::{CheckedAdd, CheckedSub};
use sp_arithmetic::{FixedPointNumber, FixedU128};
use sp_std::ops::{Add, Deref};

/// Asset state representation including asset pool reserve.
#[derive(Clone, Default, Debug)]
pub struct AssetReserveState<Balance> {
	/// Quantity of asset in omnipool
	pub reserve: Balance,
	/// Quantity of Hub Asset matching this asset
	pub hub_reserve: Balance,
	/// Quantity of LP shares for this asset
	pub shares: Balance,
	/// Quantity of LP shares for this asset owned by protocol
	pub protocol_shares: Balance,
}

impl<Balance> AssetReserveState<Balance>
where
	Balance: Into<<FixedU128 as FixedPointNumber>::Inner> + Copy + CheckedAdd + CheckedSub + Default,
{
	/// Returns price in hub asset as rational number.
	pub(crate) fn price_as_rational(&self) -> (Balance, Balance) {
		(self.hub_reserve, self.reserve)
	}

	/// Calculate price for actual state
	pub(crate) fn price(&self) -> Option<FixedU128> {
		FixedU128::checked_from_rational(self.hub_reserve.into(), self.reserve.into())
	}

	/// Update current asset state with given delta changes.
	pub fn delta_update(self, delta: &AssetStateChange<Balance>) -> Option<Self> {
		Some(Self {
			reserve: (delta.delta_reserve + self.reserve)?,
			hub_reserve: (delta.delta_hub_reserve + self.hub_reserve)?,
			shares: (delta.delta_shares + self.shares)?,
			protocol_shares: (delta.delta_protocol_shares + self.protocol_shares)?,
		})
	}
}

/// Indicates whether delta amount should be added or subtracted.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BalanceUpdate<Balance> {
	Increase(Balance),
	Decrease(Balance),
}

impl<Balance: CheckedAdd + CheckedSub + PartialOrd + Copy + Default> BalanceUpdate<Balance> {
	/// Merge two update together
	pub fn merge(self, other: Self) -> Option<Self> {
		self.checked_add(&other)
	}
}

/// The addition operator + for BalanceUpdate.
///
/// Panics if overflows in debug builds, in non-debug debug it wraps instead. Use `checked_add` for safe operation.
///
/// # Example
///
/// ```
/// use crate::hydra_dx_math::omnipool::types::BalanceUpdate;
/// assert_eq!(BalanceUpdate::Increase(100) + BalanceUpdate::Increase(100), BalanceUpdate::Increase(200));
/// ```
impl<Balance: CheckedAdd + CheckedSub + PartialOrd + Default> Add<Self> for BalanceUpdate<Balance> {
	type Output = Self;

	fn add(self, rhs: Self) -> Self::Output {
		match (self, rhs) {
			(Increase(a), Increase(b)) => BalanceUpdate::Increase(a + b),
			(Decrease(a), Decrease(b)) => BalanceUpdate::Decrease(a + b),
			(Increase(a), Decrease(b)) => {
				if a >= b {
					BalanceUpdate::Increase(a - b)
				} else {
					BalanceUpdate::Decrease(b - a)
				}
			}
			(Decrease(a), Increase(b)) => {
				if a >= b {
					BalanceUpdate::Decrease(a - b)
				} else {
					BalanceUpdate::Increase(b - a)
				}
			}
		}
	}
}

/// Performs addition that returns `None` instead of wrapping around on overflow
impl<Balance: CheckedAdd + CheckedSub + PartialOrd + Copy + Default> CheckedAdd for BalanceUpdate<Balance> {
	fn checked_add(&self, v: &Self) -> Option<Self> {
		match (self, v) {
			(Increase(a), Increase(b)) => Some(BalanceUpdate::Increase(a.checked_add(b)?)),
			(Decrease(a), Decrease(b)) => Some(BalanceUpdate::Decrease(a.checked_add(b)?)),
			(Increase(a), Decrease(b)) => {
				if a >= b {
					Some(BalanceUpdate::Increase(a.checked_sub(b)?))
				} else {
					Some(BalanceUpdate::Decrease(b.checked_sub(a)?))
				}
			}
			(Decrease(a), Increase(b)) => {
				if a >= b {
					Some(BalanceUpdate::Decrease(a.checked_sub(b)?))
				} else {
					Some(BalanceUpdate::Increase(b.checked_sub(a)?))
				}
			}
		}
	}
}

impl<Balance: Default> Default for BalanceUpdate<Balance> {
	fn default() -> Self {
		BalanceUpdate::Increase(Balance::default())
	}
}

impl<Balance: Default> Deref for BalanceUpdate<Balance> {
	type Target = Balance;

	fn deref(&self) -> &Self::Target {
		match self {
			Increase(amount) | Decrease(amount) => amount,
		}
	}
}

impl<Balance: Into<<FixedU128 as FixedPointNumber>::Inner> + CheckedAdd + CheckedSub + Copy + Default> Add<Balance>
	for BalanceUpdate<Balance>
{
	type Output = Option<Balance>;

	fn add(self, rhs: Balance) -> Self::Output {
		match &self {
			BalanceUpdate::Increase(amount) => rhs.checked_add(amount),
			BalanceUpdate::Decrease(amount) => rhs.checked_sub(amount),
		}
	}
}

/// Delta changes of asset state
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct AssetStateChange<Balance>
where
	Balance: Default,
{
	pub delta_reserve: BalanceUpdate<Balance>,
	pub delta_hub_reserve: BalanceUpdate<Balance>,
	pub delta_shares: BalanceUpdate<Balance>,
	pub delta_protocol_shares: BalanceUpdate<Balance>,
}

/// Information about trade fee amounts
#[derive(Default, Debug, PartialEq, Eq)]
pub struct TradeFee<Balance> {
	pub asset_fee: Balance,
	pub protocol_fee: Balance,
}

/// Delta changes after a trade is executed
#[derive(Default, Debug, PartialEq, Eq)]
pub struct TradeStateChange<Balance>
where
	Balance: Default,
{
	pub asset_in: AssetStateChange<Balance>,
	pub asset_out: AssetStateChange<Balance>,
	pub delta_imbalance: BalanceUpdate<Balance>,
	pub hdx_hub_amount: Balance,
	pub fee: TradeFee<Balance>,
}

/// Delta changes after a trade with hub asset is executed.
#[derive(Default, Debug)]
pub struct HubTradeStateChange<Balance>
where
	Balance: Default,
{
	pub asset: AssetStateChange<Balance>,
	pub delta_imbalance: BalanceUpdate<Balance>,
	pub fee: TradeFee<Balance>,
}

/// Delta changes after add or remove liquidity.
#[derive(Default)]
pub struct LiquidityStateChange<Balance>
where
	Balance: Default,
{
	pub asset: AssetStateChange<Balance>,
	pub delta_imbalance: BalanceUpdate<Balance>,
	pub delta_position_reserve: BalanceUpdate<Balance>,
	pub delta_position_shares: BalanceUpdate<Balance>,
	pub lp_hub_amount: Balance,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Position<Balance> {
	/// Amount of asset added to omnipool
	pub amount: Balance,
	/// Quantity of LP shares owned by LP
	pub shares: Balance,
	/// Price at which liquidity was provided
	pub price: (Balance, Balance),
}

impl<Balance> Position<Balance>
where
	Balance: Into<<FixedU128 as FixedPointNumber>::Inner> + Copy + CheckedAdd + CheckedSub + Default,
{
	pub fn price(&self) -> Option<FixedU128> {
		FixedU128::checked_from_rational(self.price.0.into(), self.price.1.into())
	}
}

#[derive(Clone, Copy, Debug)]
pub struct I129<Balance> {
	pub value: Balance,
	pub negative: bool,
}

#[cfg(test)]
mod tests {
	use super::BalanceUpdate;
	use super::CheckedAdd;
	//use cool_asserts::assert_panics;
	use test_case::test_case;

	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(100), BalanceUpdate::Increase(200) ; "When both increase")]
	#[test_case(BalanceUpdate::Increase(500), BalanceUpdate::Decrease(300), BalanceUpdate::Increase(200) ; "When increase and decrease")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(300), BalanceUpdate::Decrease(200) ; "When increase and decrease larger")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(0), BalanceUpdate::Increase(100) ; "When increase and increase by zero")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(0), BalanceUpdate::Increase(100) ; "When increase and decrease by zero")]
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(100) ; "When increase zero and decrease ")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(300), BalanceUpdate::Decrease(400) ; "When both decrease ")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(100), BalanceUpdate::Decrease(100) ; "When decrease and increase")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(300), BalanceUpdate::Increase(100) ; "When decrease and increase larger")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(0), BalanceUpdate::Decrease(200) ; "When decrease and increase zero")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(100) ; "When decrease zero and decreaes ")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(100), BalanceUpdate::Increase(0) ; "When decrease and decrease same amount ")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Increase(100), BalanceUpdate::Decrease(0) ; "When decrease and decrease same amount swapped ")] // should be probably same as previous ?
	#[test_case(BalanceUpdate::Increase(u32::MAX), BalanceUpdate::Decrease(1), BalanceUpdate::Increase(u32::MAX - 1) ; "When increase max and decrease one")]
	//#[test_case(BalanceUpdate::Increase(u32::MAX), BalanceUpdate::Increase(1), BalanceUpdate::Increase(u32::MAX - 1) ; "When increase overflows")]
	fn balance_update_add(x: BalanceUpdate<u32>, y: BalanceUpdate<u32>, result: BalanceUpdate<u32>) {
		assert_eq!(x + y, result);
	}

	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(100), Some(BalanceUpdate::Increase(200)) ; "When both increase")]
	#[test_case(BalanceUpdate::Increase(500), BalanceUpdate::Decrease(300), Some(BalanceUpdate::Increase(200)) ; "When increase and decrease")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(300), Some(BalanceUpdate::Decrease(200)) ; "When increase and decrease larger")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(0), Some(BalanceUpdate::Increase(100)) ; "When increase and increase by zero")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(0), Some(BalanceUpdate::Increase(100)) ; "When increase and decrease by zero")]
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Decrease(100), Some(BalanceUpdate::Decrease(100)) ; "When increase zero and decrease ")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(300), Some(BalanceUpdate::Decrease(400)) ; "When both decrease ")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(100), Some(BalanceUpdate::Decrease(100)) ; "When decrease and increase")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(300), Some(BalanceUpdate::Increase(100)) ; "When decrease and increase larger")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(0), Some(BalanceUpdate::Decrease(200)) ; "When decrease and increase zero")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Decrease(100), Some(BalanceUpdate::Decrease(100)) ; "When decrease zero and decreaes ")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(100), Some(BalanceUpdate::Increase(0)) ; "When decrease and decrease same amount ")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Increase(100), Some(BalanceUpdate::Decrease(0)) ; "When decrease and decrease same amount swapped ")] // should be probably same as previous ?
	#[test_case(BalanceUpdate::Increase(u32::MAX), BalanceUpdate::Decrease(1), Some(BalanceUpdate::Increase(u32::MAX - 1)) ; "When increase max and decrease one")]
	#[test_case(BalanceUpdate::Increase(u32::MAX), BalanceUpdate::Increase(1), None ; "When increase overflows")]
	#[test_case(BalanceUpdate::Decrease(u32::MAX), BalanceUpdate::Decrease(1), None ; "When decrease overflows")]
	fn balance_update_checked_add(x: BalanceUpdate<u32>, y: BalanceUpdate<u32>, result: Option<BalanceUpdate<u32>>) {
		assert_eq!(x.checked_add(&y), result);
	}

	#[test]
	fn balance_update_to_balance_addition_works() {
		let zero = 0u32;
		assert_eq!(BalanceUpdate::Increase(100u32) + 200u32, Some(300));
		assert_eq!(BalanceUpdate::Decrease(50u32) + 100u32, Some(50));
		assert_eq!(BalanceUpdate::Decrease(50u32) + 50u32, Some(0));
		assert_eq!(BalanceUpdate::Decrease(50u32) + zero, None);
		assert_eq!(BalanceUpdate::Increase(50u32) + zero, Some(50));
		assert_eq!(BalanceUpdate::Decrease(100u32) + 50u32, None);
	}
}
