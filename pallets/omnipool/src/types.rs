use super::*;
use crate::types::BalanceUpdate::{Decrease, Increase};
use frame_support::pallet_prelude::*;
use sp_runtime::{FixedPointNumber, FixedU128};
use std::ops::{Add, Deref, Sub};

pub type Price = FixedU128;

#[derive(Clone, Default, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct AssetState<Balance> {
	/// Quantity of asset in omnipool
	pub(super) reserve: Balance,
	/// Quantity of Hub Asset matching this asset
	pub(super) hub_reserve: Balance,
	/// Quantity of LP shares for this asset
	pub(super) shares: Balance,
	/// Quantity of LP shares for this asset owned by protocol
	pub(super) protocol_shares: Balance,
	/// TVL of asset
	pub(super) tvl: Balance,
}

impl<Balance> AssetState<Balance>
where
	Balance: Into<<FixedU128 as FixedPointNumber>::Inner> + Copy + CheckedAdd + CheckedSub + Default,
{
	/// Calculate price for actual state
	pub(super) fn price(&self) -> FixedU128 {
		FixedU128::from((self.hub_reserve.into(), self.reserve.into()))
	}

	/// Update current asset state with given delta changes.
	pub(super) fn delta_update(&mut self, delta: &AssetStateChange<Balance>) -> Option<()> {
		self.reserve = update_value!(self.reserve, delta.delta_reserve)?;
		self.hub_reserve = update_value!(self.hub_reserve, delta.delta_hub_reserve)?;
		self.shares = update_value!(self.shares, delta.delta_shares)?;
		self.protocol_shares = update_value!(self.protocol_shares, delta.delta_protocol_shares)?;
		self.tvl = update_value!(self.tvl, delta.delta_tvl)?;
		Some(())
	}
}

/// Position in Omnipool represents a moment when LP provided liquidity of an asset at that moment’s price.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Position<Balance, AssetId> {
	/// Provided Asset
	pub(super) asset_id: AssetId,
	/// Amount of asset added to omnipool
	pub(super) amount: Balance,
	/// Quantity of LP shares owned by LP
	pub(super) shares: Balance,
	/// Price at which liquidity was provided
	// TODO: Due to missing MaxEncodedLen impl for FixedU128, it is not possible to use that type in storage
	// This can change in 0.9.17 where the missing trait is implemented
	// And FixedU128 can be use instead.
	pub(super) price: Balance,
}

impl<Balance, AssetId> Position<Balance, AssetId>
where
	Balance: From<u128> + Into<u128> + Copy + CheckedAdd + CheckedSub + Default,
{
	// Storing position price as Balance type.
	// Let's convert `Balance` into FixedU128 and vice versa
	pub(super) fn fixed_price(&self) -> Price {
		Price::from_inner(self.price.into())
	}

	// Due to missing MaxEncodedLen impl for FixedU128, it is not possible to use that type in storage
	// This can change in 0.9.17 where the missing trait is implemented
	// and there won't be a need to convert it to balance for storage.
	pub(super) fn price_to_balance(price: Price) -> Balance {
		price.into_inner().into()
	}

	/// Update current position state with given delta changes.
	pub(super) fn delta_update(
		&mut self,
		delta_reserve: &BalanceUpdate<Balance>,
		delta_shares: &BalanceUpdate<Balance>,
	) -> Option<()> {
		self.amount = update_value!(self.amount, delta_reserve)?;
		self.shares = update_value!(self.shares, delta_shares)?;
		Some(())
	}
}

/// Simple type to represent imbalance which can be positive or negative.
// Note: Simple prefix is used not to confuse with Imbalance trait from frame_support.
#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub(crate) struct SimpleImbalance<Balance: Copy> {
	pub value: Balance,
	pub negative: bool,
}

impl<Balance: Default + Copy> Default for SimpleImbalance<Balance> {
	fn default() -> Self {
		Self {
			value: Balance::default(),
			negative: true,
		}
	}
}

/// The addition operator + for SimpleImbalance.
///
/// Adds amount to imbalance.
///
/// Note that it returns Option<self> rather than Self.
///
/// Note: Implements `Add` instead of `CheckedAdd` because `CheckedAdd` requires the second parameter
/// to be the same type as the first while we want to add a `Balance` here.
///
/// # Example
///
/// ```ignore
/// let imbalance = SimpleImbalance{value: 100, negative: false} ;
/// assert_eq!(imbalance + 200 , Some(SimpleImbalance{value: 300, negative: false}));
/// ```
impl<Balance: CheckedAdd + CheckedSub + PartialOrd + Copy> Add<Balance> for SimpleImbalance<Balance> {
	type Output = Option<Self>;

	fn add(self, amount: Balance) -> Self::Output {
		let (value, sign) = if !self.negative {
			(self.value.checked_add(&amount)?, self.negative)
		} else if self.value < amount {
			(amount.checked_sub(&self.value)?, false)
		} else {
			(self.value.checked_sub(&amount)?, self.negative)
		};
		Some(Self { value, negative: sign })
	}
}

/// The subtraction operator - for SimpleImbalance.
///
/// Subtracts amount from imbalance.
///
/// Note that it returns Option<self> rather than Self.
///
/// # Example
///
/// ```ignore
/// let imbalance = SimpleImbalance{value: 200, negative: false} ;
/// assert_eq!(imbalance - 100 , Some(SimpleImbalance{value: 100, negative: false}));
/// ```
impl<Balance: CheckedAdd + CheckedSub + PartialOrd + Copy> Sub<Balance> for SimpleImbalance<Balance> {
	type Output = Option<Self>;

	fn sub(self, amount: Balance) -> Self::Output {
		let (value, sign) = if self.negative {
			(self.value.checked_add(&amount)?, self.negative)
		} else if self.value < amount {
			(amount.checked_sub(&self.value)?, true)
		} else {
			(self.value.checked_sub(&amount)?, self.negative)
		};
		Some(Self { value, negative: sign })
	}
}

/// Indicates whether hub asset changes the total issuance or not.
#[derive(PartialOrd, PartialEq)]
pub(super) enum HubAssetIssuanceUpdate {
	AdjustSupply,
	JustTransfer,
}

/// Indicates whether delta amount should be added or subtracted.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum BalanceUpdate<Balance> {
	Increase(Balance),
	Decrease(Balance),
}

impl<Balance: CheckedAdd + CheckedSub + PartialOrd + Copy + Default> BalanceUpdate<Balance> {
	/// Merge two update together
	pub(crate) fn merge(self, other: Self) -> Option<Self> {
		self.checked_add(&other)
	}
}

/// The addition operator + for BalanceUpdate.
///
/// Panics if overflows in debug builds, in non-debug debug it wraps instead.. Use checked_add for safe operation.
///
/// # Example
///
/// ```ignore
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

#[macro_export]
macro_rules! update_value {
	( $x:expr, $y:expr) => {{
		match &$y {
			BalanceUpdate::Increase(amount) => $x.checked_add(&amount),
			BalanceUpdate::Decrease(amount) => $x.checked_sub(&amount),
		}
	}};
}

/// Delta changes of asset state
#[derive(Default, Copy, Clone, Debug)]
pub(super) struct AssetStateChange<Balance>
where
	Balance: Default + Copy,
{
	pub(crate) delta_reserve: BalanceUpdate<Balance>,
	pub(crate) delta_hub_reserve: BalanceUpdate<Balance>,
	pub(crate) delta_shares: BalanceUpdate<Balance>,
	pub(crate) delta_protocol_shares: BalanceUpdate<Balance>,
	pub(crate) delta_tvl: BalanceUpdate<Balance>,
}

/// Delta changes after a trade is executed
#[derive(Default, Copy, Clone)]
pub(super) struct TradeStateChange<Balance>
where
	Balance: Default + Copy,
{
	pub(crate) asset_in: AssetStateChange<Balance>,
	pub(crate) asset_out: AssetStateChange<Balance>,
	pub(crate) delta_imbalance: BalanceUpdate<Balance>,
	pub(crate) hdx_hub_amount: Balance,
}

/// Delta changes after a trade with hub asset is executed.
#[derive(Default, Copy, Clone)]
pub(super) struct HubTradeStateChange<Balance>
where
	Balance: Default + Copy,
{
	pub(crate) asset: AssetStateChange<Balance>,
	pub(crate) delta_imbalance: BalanceUpdate<Balance>,
}

/// Delta changes after add or remove liquidity.
#[derive(Default, Copy, Clone, Debug)]
pub(super) struct LiquidityStateChange<Balance>
where
	Balance: Default + Copy,
{
	pub(crate) asset: AssetStateChange<Balance>,
	pub(crate) delta_imbalance: BalanceUpdate<Balance>,
	pub(crate) delta_position_reserve: BalanceUpdate<Balance>,
	pub(crate) delta_position_shares: BalanceUpdate<Balance>,
	pub(crate) lp_hub_amount: Balance,
}

#[cfg(test)]
mod tests {
	use super::BalanceUpdate;
	use super::CheckedAdd;
	use super::SimpleImbalance;
	use cool_asserts::assert_panics;
	#[test]
	fn simple_imbalance_addition_works() {
		assert_eq!(
			SimpleImbalance {
				value: 100,
				negative: false
			} + 200,
			Some(SimpleImbalance {
				value: 300,
				negative: false
			})
		);
		assert_eq!(
			SimpleImbalance {
				value: 100,
				negative: true
			} + 200,
			Some(SimpleImbalance {
				value: 100,
				negative: false
			})
		);
		assert_eq!(
			SimpleImbalance {
				value: 500,
				negative: true
			} + 200,
			Some(SimpleImbalance {
				value: 300,
				negative: true
			})
		);

		assert_eq!(
			SimpleImbalance {
				value: 500,
				negative: true
			} + 500,
			Some(SimpleImbalance {
				value: 0,
				negative: true
			})
		);
		assert_eq!(
			SimpleImbalance {
				value: 0,
				negative: true
			} + 500,
			Some(SimpleImbalance {
				value: 500,
				negative: false
			})
		);
		assert_eq!(
			SimpleImbalance {
				value: 0,
				negative: false
			} + 500,
			Some(SimpleImbalance {
				value: 500,
				negative: false
			})
		);

		assert_eq!(
			SimpleImbalance {
				value: 1u128,
				negative: true
			} + u128::MAX,
			Some(SimpleImbalance {
				value: u128::MAX - 1,
				negative: false
			})
		);

		assert_eq!(
			SimpleImbalance {
				value: u128::MAX,
				negative: false
			} + 1,
			None
		);
		assert_eq!(
			SimpleImbalance {
				value: 1u128,
				negative: false
			} + u128::MAX,
			None
		);
	}

	#[test]
	fn simple_imbalance_subtraction_works() {
		assert_eq!(
			SimpleImbalance {
				value: 200,
				negative: false
			} - 300,
			Some(SimpleImbalance {
				value: 100,
				negative: true
			})
		);
		assert_eq!(
			SimpleImbalance {
				value: 200,
				negative: true
			} - 300,
			Some(SimpleImbalance {
				value: 500,
				negative: true
			})
		);
		assert_eq!(
			SimpleImbalance {
				value: 300,
				negative: false
			} - 300,
			Some(SimpleImbalance {
				value: 0,
				negative: false
			})
		);
		assert_eq!(
			SimpleImbalance {
				value: 0,
				negative: false
			} - 300,
			Some(SimpleImbalance {
				value: 300,
				negative: true
			})
		);
		assert_eq!(
			SimpleImbalance {
				value: 0,
				negative: true
			} - 300,
			Some(SimpleImbalance {
				value: 300,
				negative: true
			})
		);

		assert_eq!(
			SimpleImbalance {
				value: 1u128,
				negative: false
			} - u128::MAX,
			Some(SimpleImbalance {
				value: u128::MAX - 1,
				negative: true
			})
		);

		assert_eq!(
			SimpleImbalance {
				value: u128::MAX,
				negative: true
			} - 1,
			None
		);
		assert_eq!(
			SimpleImbalance {
				value: 1u128,
				negative: true
			} - u128::MAX,
			None
		);
	}

	#[test]
	fn balance_update_addition_works() {
		assert_eq!(
			BalanceUpdate::Increase(100) + BalanceUpdate::Increase(100),
			BalanceUpdate::Increase(200)
		);
		assert_eq!(
			BalanceUpdate::Increase(500) + BalanceUpdate::Decrease(300),
			BalanceUpdate::Increase(200)
		);
		assert_eq!(
			BalanceUpdate::Increase(100) + BalanceUpdate::Decrease(300),
			BalanceUpdate::Decrease(200)
		);
		assert_eq!(
			BalanceUpdate::Increase(100) + BalanceUpdate::Decrease(0),
			BalanceUpdate::Increase(100)
		);
		assert_eq!(
			BalanceUpdate::Increase(0) + BalanceUpdate::Decrease(100),
			BalanceUpdate::Decrease(100)
		);

		assert_eq!(
			BalanceUpdate::Decrease(100) + BalanceUpdate::Decrease(300),
			BalanceUpdate::Decrease(400)
		);
		assert_eq!(
			BalanceUpdate::Decrease(200) + BalanceUpdate::Increase(100),
			BalanceUpdate::Decrease(100)
		);
		assert_eq!(
			BalanceUpdate::Decrease(200) + BalanceUpdate::Increase(300),
			BalanceUpdate::Increase(100)
		);
		assert_eq!(
			BalanceUpdate::Decrease(200) + BalanceUpdate::Increase(0),
			BalanceUpdate::Decrease(200)
		);
		assert_eq!(
			BalanceUpdate::Decrease(0) + BalanceUpdate::Decrease(100),
			BalanceUpdate::Decrease(100)
		);

		assert_eq!(
			BalanceUpdate::Increase(100) + BalanceUpdate::Decrease(100),
			BalanceUpdate::Increase(0)
		);
		assert_eq!(
			BalanceUpdate::Decrease(100) + BalanceUpdate::Increase(100),
			BalanceUpdate::Decrease(0)
		);
		assert_eq!(
			BalanceUpdate::Increase(0) + BalanceUpdate::Decrease(0),
			BalanceUpdate::Increase(0)
		);
		assert_eq!(
			BalanceUpdate::Decrease(0) + BalanceUpdate::Increase(0),
			BalanceUpdate::Decrease(0)
		);

		assert_eq!(
			BalanceUpdate::Increase(u128::MAX) + BalanceUpdate::Decrease(1),
			BalanceUpdate::Increase(u128::MAX - 1)
		);

		assert_panics!(BalanceUpdate::Increase(u128::MAX) + BalanceUpdate::Increase(1));
		assert_panics!(BalanceUpdate::Decrease(u128::MAX) + BalanceUpdate::Decrease(1));
	}
	#[test]
	fn balance_update_safe_addition_works() {
		assert_eq!(
			BalanceUpdate::Increase(100).checked_add(&BalanceUpdate::Increase(100)),
			Some(BalanceUpdate::Increase(200))
		);
		assert_eq!(
			BalanceUpdate::Increase(500).checked_add(&BalanceUpdate::Decrease(300)),
			Some(BalanceUpdate::Increase(200))
		);
		assert_eq!(
			BalanceUpdate::Increase(100).checked_add(&BalanceUpdate::Decrease(300)),
			Some(BalanceUpdate::Decrease(200))
		);

		assert_eq!(
			BalanceUpdate::Increase(100).checked_add(&BalanceUpdate::Decrease(0)),
			Some(BalanceUpdate::Increase(100))
		);
		assert_eq!(
			BalanceUpdate::Increase(0).checked_add(&BalanceUpdate::Decrease(100)),
			Some(BalanceUpdate::Decrease(100))
		);

		assert_eq!(
			BalanceUpdate::Decrease(100).checked_add(&BalanceUpdate::Decrease(300)),
			Some(BalanceUpdate::Decrease(400))
		);
		assert_eq!(
			BalanceUpdate::Decrease(200).checked_add(&BalanceUpdate::Increase(100)),
			Some(BalanceUpdate::Decrease(100))
		);
		assert_eq!(
			BalanceUpdate::Decrease(200).checked_add(&BalanceUpdate::Increase(300)),
			Some(BalanceUpdate::Increase(100))
		);
		assert_eq!(
			BalanceUpdate::Decrease(200).checked_add(&BalanceUpdate::Increase(0)),
			Some(BalanceUpdate::Decrease(200))
		);
		assert_eq!(
			BalanceUpdate::Decrease(0).checked_add(&BalanceUpdate::Decrease(100)),
			Some(BalanceUpdate::Decrease(100))
		);

		assert_eq!(
			BalanceUpdate::Increase(100).checked_add(&BalanceUpdate::Decrease(100)),
			Some(BalanceUpdate::Increase(0))
		);
		assert_eq!(
			BalanceUpdate::Decrease(100).checked_add(&BalanceUpdate::Increase(100)),
			Some(BalanceUpdate::Decrease(0))
		);
		assert_eq!(
			BalanceUpdate::Increase(0).checked_add(&BalanceUpdate::Decrease(0)),
			Some(BalanceUpdate::Increase(0))
		);
		assert_eq!(
			BalanceUpdate::Decrease(0).checked_add(&BalanceUpdate::Increase(0)),
			Some(BalanceUpdate::Decrease(0))
		);

		assert_eq!(
			BalanceUpdate::Increase(u128::MAX).checked_add(&BalanceUpdate::Decrease(1)),
			Some(BalanceUpdate::Increase(u128::MAX - 1))
		);

		assert_eq!(
			BalanceUpdate::Increase(u128::MAX).checked_add(&BalanceUpdate::Increase(1)),
			None
		);
		assert_eq!(
			BalanceUpdate::Decrease(u128::MAX).checked_add(&BalanceUpdate::Decrease(1)),
			None
		);
	}
}
