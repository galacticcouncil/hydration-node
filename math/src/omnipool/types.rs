use crate::omnipool::calculate_burn_amount_based_on_fee_taken;
use crate::omnipool::types::BalanceUpdate::{Decrease, Increase};
use codec::{Decode, Encode, MaxEncodedLen};
use num_traits::{CheckedAdd, CheckedMul, CheckedSub, SaturatingAdd, Zero};
use scale_info::TypeInfo;
use sp_arithmetic::traits::Saturating;
use sp_arithmetic::{FixedPointNumber, FixedPointOperand, FixedU128};
use sp_std::cmp::Ordering;
use sp_std::ops::{Add, Deref, Mul, Sub};

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
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Copy, Clone, Debug, PartialEq, Eq)]
pub enum BalanceUpdate<Balance> {
	Increase(Balance),
	Decrease(Balance),
}

impl<Balance: CheckedAdd + CheckedSub + PartialOrd + Copy + Default + Saturating + Copy + Zero> BalanceUpdate<Balance> {
	/// Merge two updates.
	pub fn merge(self, other: Self) -> Option<Self> {
		self.checked_add(&other)
	}

	pub fn saturating_merge(self, other: Self) -> Self {
		self.saturating_add(&other)
	}

	pub fn is_positive(self) -> bool {
		match self {
			Increase(_) => true,
			Decrease(v) => v.is_zero(),
		}
	}

	// Note: CheckedMul trait can't be implemented for distinct types, so we implement it here.
	pub fn checked_mul_fixed(self, other: FixedU128) -> Option<Self>
	where
		Balance: FixedPointOperand,
	{
		match self {
			Increase(v) => Some(Increase(other.checked_mul_int(v)?)),
			Decrease(v) => Some(Decrease(other.checked_mul_int(v)?)),
		}
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
			(Increase(a), Increase(b)) => Increase(a + b),
			(Decrease(a), Decrease(b)) => Decrease(a + b),
			(Increase(a), Decrease(b)) => {
				if a >= b {
					Increase(a - b)
				} else {
					Decrease(b - a)
				}
			}
			(Decrease(a), Increase(b)) => {
				if a >= b {
					Decrease(a - b)
				} else {
					Increase(b - a)
				}
			}
		}
	}
}

impl<Balance: Copy + CheckedAdd + CheckedSub + PartialOrd + Default + Saturating> SaturatingAdd
	for BalanceUpdate<Balance>
{
	fn saturating_add(&self, rhs: &Self) -> Self {
		match (self, rhs) {
			(Increase(a), Increase(b)) => Increase(a.saturating_add(*b)),
			(Decrease(a), Decrease(b)) => Decrease(a.saturating_add(*b)),
			(Increase(a), Decrease(b)) => {
				if a >= b {
					Increase(a.saturating_sub(*b))
				} else {
					Decrease(b.saturating_sub(*a))
				}
			}
			(Decrease(a), Increase(b)) => {
				if a >= b {
					Decrease(a.saturating_sub(*b))
				} else {
					Increase(b.saturating_sub(*a))
				}
			}
		}
	}
}

/// Performs addition that returns `None` instead of wrapping around on overflow
impl<Balance: CheckedAdd + CheckedSub + PartialOrd + Copy + Default> CheckedAdd for BalanceUpdate<Balance> {
	fn checked_add(&self, v: &Self) -> Option<Self> {
		match (self, v) {
			(Increase(a), Increase(b)) => Some(Increase(a.checked_add(b)?)),
			(Decrease(a), Decrease(b)) => Some(Decrease(a.checked_add(b)?)),
			(Increase(a), Decrease(b)) => {
				if a >= b {
					Some(Increase(a.checked_sub(b)?))
				} else {
					Some(Decrease(b.checked_sub(a)?))
				}
			}
			(Decrease(a), Increase(b)) => {
				if a >= b {
					Some(Decrease(a.checked_sub(b)?))
				} else {
					Some(Increase(b.checked_sub(a)?))
				}
			}
		}
	}
}

impl<Balance: CheckedAdd + CheckedSub + PartialOrd + Zero + Default> Sub<Self> for BalanceUpdate<Balance> {
	type Output = Self;

	fn sub(self, rhs: Self) -> Self::Output {
		match (self, rhs) {
			(Increase(a), Increase(b)) => {
				if a >= b {
					Increase(a - b)
				} else {
					Decrease(b - a)
				}
			}
			(Decrease(a), Decrease(b)) => {
				if a == b {
					Increase(Zero::zero())
				} else if a > b {
					Decrease(a - b)
				} else {
					Increase(b - a)
				}
			}
			(Increase(a), Decrease(b)) => Increase(a + b),
			(Decrease(a), Increase(b)) => {
				if a.is_zero() && b.is_zero() {
					Increase(Zero::zero())
				} else {
					Decrease(a + b)
				}
			}
		}
	}
}

impl<Balance: CheckedAdd + CheckedSub + PartialOrd + Zero + Copy + Default> CheckedSub for BalanceUpdate<Balance> {
	fn checked_sub(&self, v: &Self) -> Option<Self> {
		match (self, v) {
			(Increase(a), Increase(b)) => {
				if a >= b {
					Some(Increase(a.checked_sub(b)?))
				} else {
					Some(Decrease(b.checked_sub(a)?))
				}
			}
			(Decrease(a), Decrease(b)) => {
				if a == b {
					Some(Increase(Zero::zero()))
				} else if a > b {
					Some(Decrease(a.checked_sub(b)?))
				} else {
					Some(Increase(b.checked_sub(a)?))
				}
			}
			(Increase(a), Decrease(b)) => Some(Increase(a.checked_add(b)?)),
			(Decrease(a), Increase(b)) => {
				if a.is_zero() && b.is_zero() {
					Some(Increase(Zero::zero()))
				} else {
					Some(Decrease(a.checked_add(b)?))
				}
			}
		}
	}
}

impl<Balance: Mul<Output = Balance>> Mul for BalanceUpdate<Balance> {
	type Output = Self;
	fn mul(self, v: Self) -> Self {
		match (self, v) {
			(Increase(a), Increase(b)) => Increase(a * b),
			(Decrease(a), Decrease(b)) => Increase(a * b),
			(Increase(a), Decrease(b)) => Decrease(a * b),
			(Decrease(a), Increase(b)) => Decrease(a * b),
		}
	}
}

impl<Balance: CheckedMul> CheckedMul for BalanceUpdate<Balance> {
	fn checked_mul(&self, v: &Self) -> Option<Self> {
		match (self, v) {
			(Increase(a), Increase(b)) => Some(Increase(a.checked_mul(b)?)),
			(Decrease(a), Decrease(b)) => Some(Increase(a.checked_mul(b)?)),
			(Increase(a), Decrease(b)) => Some(Decrease(a.checked_mul(b)?)),
			(Decrease(a), Increase(b)) => Some(Decrease(a.checked_mul(b)?)),
		}
	}
}

impl<Balance: Default> Default for BalanceUpdate<Balance> {
	fn default() -> Self {
		Increase(Balance::default())
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
			Increase(amount) => rhs.checked_add(amount),
			Decrease(amount) => rhs.checked_sub(amount),
		}
	}
}

impl<Balance: Ord + Zero> PartialOrd for BalanceUpdate<Balance> {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl<Balance: Ord + Zero> Ord for BalanceUpdate<Balance> {
	fn cmp(&self, other: &Self) -> Ordering {
		match (self, other) {
			(Increase(a), Increase(b)) => a.cmp(b),
			(Decrease(a), Decrease(b)) => b.cmp(a),
			(Increase(a), Decrease(b)) => {
				if a.is_zero() && b.is_zero() {
					Ordering::Equal
				} else {
					Ordering::Greater
				}
			}
			(Decrease(a), Increase(b)) => {
				if a.is_zero() && b.is_zero() {
					Ordering::Equal
				} else {
					Ordering::Less
				}
			}
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
	pub extra_hub_reserve_amount: BalanceUpdate<Balance>,
}
impl<
		Balance: Into<<FixedU128 as FixedPointNumber>::Inner>
			+ CheckedAdd
			+ CheckedSub
			+ Copy
			+ Default
			+ PartialOrd
			+ sp_std::fmt::Debug
			+ Saturating
			+ Zero,
	> AssetStateChange<Balance>
{
	pub fn total_delta_hub_reserve(&self) -> BalanceUpdate<Balance> {
		self.delta_hub_reserve.saturating_merge(self.extra_hub_reserve_amount)
	}

	fn account_for_fee_taken(self, amt_to_burn: Balance) -> Self {
		let mut v = self;
		debug_assert!(
			*v.extra_hub_reserve_amount >= amt_to_burn,
			"Amount to burn {:?} > to mint {:?}",
			amt_to_burn,
			v.extra_hub_reserve_amount
		);
		v.extra_hub_reserve_amount = v.extra_hub_reserve_amount.saturating_merge(Decrease(amt_to_burn));
		v
	}
}

/// Information about trade fee amounts
#[derive(Default, Debug, PartialEq, Eq)]
pub struct TradeFee<Balance> {
	pub asset_fee: Balance,
	// Total protocol fee amount ( includes burned portion)
	pub protocol_fee: Balance,
	// Burned portion of protocol fee amount
	pub burned_protocol_fee: Balance,
}

/// Delta changes after a trade is executed
#[derive(Default, Debug, PartialEq, Eq)]
pub struct TradeStateChange<Balance>
where
	Balance: Default,
{
	pub asset_in: AssetStateChange<Balance>,
	pub asset_out: AssetStateChange<Balance>,
	pub fee: TradeFee<Balance>,
}

impl TradeStateChange<crate::types::Balance> {
	pub fn account_for_fee_taken(self, taken_fee: crate::types::Balance) -> Self {
		let mut v = self;
		let extra_amt =
			calculate_burn_amount_based_on_fee_taken(taken_fee, v.fee.asset_fee, *v.asset_out.extra_hub_reserve_amount);
		v.asset_out = v.asset_out.account_for_fee_taken(extra_amt);
		v
	}
}

/// Delta changes after a trade with hub asset is executed.
#[derive(Default, Debug)]
pub struct HubTradeStateChange<Balance>
where
	Balance: Default,
{
	pub asset: AssetStateChange<Balance>,
	pub fee: TradeFee<Balance>,
}

impl HubTradeStateChange<crate::types::Balance> {
	pub fn account_for_fee_taken(self, taken_fee: crate::types::Balance) -> Self {
		let mut v = self;
		let extra_amt =
			calculate_burn_amount_based_on_fee_taken(taken_fee, v.fee.asset_fee, *v.asset.extra_hub_reserve_amount);
		v.asset = v.asset.account_for_fee_taken(extra_amt);
		v
	}
}

/// Delta changes after add or remove liquidity.
#[derive(Default)]
pub struct LiquidityStateChange<Balance>
where
	Balance: Default,
{
	pub asset: AssetStateChange<Balance>,
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

pub mod slip_fee {
	use crate::omnipool::types::BalanceUpdate::{Decrease, Increase};
	use crate::omnipool::types::{AssetReserveState, BalanceUpdate};
	use crate::types::Balance;
	use crate::{to_balance, to_u256};
	use codec::{Decode, Encode, MaxEncodedLen};
	use num_traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, One, SaturatingAdd, Zero};
	use primitive_types::U256;
	use scale_info::TypeInfo;
	use sp_arithmetic::ArithmeticError::Overflow;
	use sp_arithmetic::{FixedPointNumber, FixedU128, Permill};
	use sp_std::vec::Vec;

	/// Hub asset state for slip fee calculation
	#[derive(Default, Encode, Decode, TypeInfo, MaxEncodedLen, Copy, Clone, Debug, Eq, PartialEq)]
	pub struct HubAssetBlockState<Balance> {
		/// Hub reserve (Q₀) at start of current block
		pub hub_reserve_at_block_start: Balance,

		/// Current net H2O delta for this asset in current block
		/// Signed value: positive = net buying, negative = net selling
		pub current_delta_hub_reserve: BalanceUpdate<Balance>,
	}
	impl HubAssetBlockState<Balance> {
		pub fn new(block_start_reserve: Balance) -> Self {
			HubAssetBlockState {
				hub_reserve_at_block_start: block_start_reserve,
				current_delta_hub_reserve: Increase(0),
			}
		}
	}

	/// Configuration for slip fee calculation
	#[derive(Default, Debug)]
	pub struct SlipFeeConfig<Balance> {
		/// Slip factor (s) - typically 0.0 to 2.0
		pub slip_factor: FixedU128,

		/// Maximum slip fee cap
		pub max_slip_fee: FixedU128,

		/// Hub asset state for asset_in
		pub hub_state_in: HubAssetBlockState<Balance>,

		/// Hub asset state for asset_out
		pub hub_state_out: HubAssetBlockState<Balance>,
	}
	impl SlipFeeConfig<Balance> {
		pub fn new_from_asset_state(
			asset_in_state: &AssetReserveState<Balance>,
			asset_out_state: &AssetReserveState<Balance>,
			slip_factor: FixedU128,
			max_slip_fee: FixedU128,
		) -> Self {
			Self {
				slip_factor,
				max_slip_fee,
				hub_state_in: HubAssetBlockState::new(asset_in_state.hub_reserve),
				hub_state_out: HubAssetBlockState::new(asset_out_state.hub_reserve),
			}
		}

		pub fn calculate_slip_fee(
			&self,
			delta: BalanceUpdate<Balance>,
			hub_reserve_at_block_start: Balance,
		) -> Option<FixedU128> {
			if self.slip_factor.is_zero() {
				// slip fee disabled
				Some(FixedU128::zero())
			} else {
				// slip factor == 1
				FixedU128::from(*delta).checked_div(&FixedU128::from(
					*(delta.checked_add(&Increase(hub_reserve_at_block_start))?),
				))
			}
		}

		pub fn calculate_slip_fee_sell(&self, delta_hub_reserve_in: Balance) -> Option<FixedU128> {
			// add new delta to existing delta
			let delta_hub_in = BalanceUpdate::<Balance>::Decrease(delta_hub_reserve_in)
				.checked_add(&self.hub_state_in.current_delta_hub_reserve)?;
			let slip_fee_sell = self.calculate_slip_fee(delta_hub_in, self.hub_state_in.hub_reserve_at_block_start)?;
			Some(sp_std::cmp::min(slip_fee_sell, self.max_slip_fee))
		}

		pub fn calculate_slip_fee_buy(&self, delta_hub_reserve_out: Balance) -> Option<FixedU128> {
			let delta_hub_out = BalanceUpdate::<Balance>::Increase(delta_hub_reserve_out)
				.checked_add(&self.hub_state_out.current_delta_hub_reserve)?;
			let slip_fee_buy = self.calculate_slip_fee(delta_hub_out, self.hub_state_out.hub_reserve_at_block_start)?;
			Some(sp_std::cmp::min(slip_fee_buy, self.max_slip_fee))
		}

		pub fn invert_buy_side_slip_fee(&self, delta_hub_reserve_out_net: Balance) -> Option<Balance> {
			let mut candidates: Vec<Balance> = Vec::new();

			let delta_hub_reserve_out_gross = if self.slip_factor.is_zero() {
				Some(delta_hub_reserve_out_net)
			} else {
				let denom = to_u256!(self
					.hub_state_out
					.hub_reserve_at_block_start
					.checked_sub(delta_hub_reserve_out_net)?);
				let n1 = to_u256!(self.hub_state_out.hub_reserve_at_block_start);
				let n2 = self
					.hub_state_out
					.current_delta_hub_reserve
					.merge(Increase(delta_hub_reserve_out_net))?;
				if n2.is_positive() {
					let u = n1.checked_mul(to_u256!(*n2))?.checked_div(denom)?;
					let u = to_balance!(u).ok()?;

					let d = Increase(u).checked_sub(&self.hub_state_out.current_delta_hub_reserve)?;
					if d.is_positive() {
						candidates.push(*d);
					}
				};

				let b2 = Increase(self.hub_state_out.hub_reserve_at_block_start)
					.checked_sub(&self.hub_state_out.current_delta_hub_reserve.checked_mul(&Increase(2))?)?
					.checked_sub(&Increase(delta_hub_reserve_out_net))?;
				let c2 = Decrease(self.hub_state_out.hub_reserve_at_block_start).checked_mul(
					&self
						.hub_state_out
						.current_delta_hub_reserve
						.checked_add(&Increase(delta_hub_reserve_out_net))?,
				)?;
				let (b2_hp, c2_hp) = to_u256!(*b2, *c2);
				let disc2_hp = if c2.is_positive() {
					b2_hp.checked_mul(b2_hp)?.checked_sub(c2_hp.checked_mul(U256::from(8))?)
				} else {
					b2_hp.checked_mul(b2_hp)?.checked_add(c2_hp.checked_mul(U256::from(8))?)
				};

				if let Some(disc2_hp) = disc2_hp {
					let mut u_candidates: Vec<BalanceUpdate<Balance>> = Vec::new();
					let sd_hp = disc2_hp.integer_sqrt();
					let sd2 = to_balance!(sd_hp).ok()?;
					let u1 = Increase(sd2).checked_sub(&b2)?;
					let u2 = Decrease(sd2).checked_sub(&b2)?;
					for u in [u1, u2] {
						if !u.is_positive() {
							u_candidates.push(Decrease((*u).checked_div(4)?));
						}
					}
					for u in u_candidates {
						if self.hub_state_out.hub_reserve_at_block_start > *u {
							let d = u.checked_sub(&self.hub_state_out.current_delta_hub_reserve)?;
							if d.is_positive() {
								candidates.push(*d);
							}
						}
					}
				}

				let mut valid_ds: Vec<Balance> = Vec::new();
				for d in candidates {
					let delta_hub_out = Increase(d).checked_add(&self.hub_state_out.current_delta_hub_reserve)?;
					let slip_fee =
						self.calculate_slip_fee(delta_hub_out, self.hub_state_out.hub_reserve_at_block_start)?;
					if slip_fee < self.max_slip_fee && slip_fee < FixedU128::one() {
						valid_ds.push(d);
					}
				}
				let result = if !valid_ds.is_empty() {
					valid_ds.iter().min().cloned()
				} else {
					let k_sat = FixedU128::one().checked_sub(&self.max_slip_fee)?;
					FixedU128::from(delta_hub_reserve_out_net)
						.checked_div(&k_sat)?
						.checked_div_int(1u128)
				};

				result
			}?;

			Some(delta_hub_reserve_out_gross)
		}

		pub fn invert_sell_side_slip_fee(
			&self,
			delta_hub_reserve_out_gross: Balance,
			protocol_fee: &Permill,
		) -> Option<Balance> {
			if self.slip_factor.is_zero() {
				return Some(
					FixedU128::from_inner(delta_hub_reserve_out_gross)
						.checked_div(&Permill::from_percent(100).checked_sub(protocol_fee)?.into())?
						.into_inner(),
				);
			}

			let k = FixedU128::one().checked_sub(&(*protocol_fee).into())?;
			let c_k = self.hub_state_in.current_delta_hub_reserve.checked_mul_fixed(k)?;

			let mut is_p_neg = false;
			let p = if Increase(delta_hub_reserve_out_gross) < c_k {
				if k >= self.slip_factor {
					k.checked_sub(&self.slip_factor)?
				} else {
					is_p_neg = true;
					self.slip_factor.checked_sub(&k)?
				}
			} else {
				k.checked_add(&self.slip_factor)?
			};

			let q1 = Increase(
				k.checked_mul_int(self.hub_state_in.hub_reserve_at_block_start)?
					.checked_add(delta_hub_reserve_out_gross)?,
			);
			let q2 = self.hub_state_in.current_delta_hub_reserve.checked_mul_fixed(p)?;
			let q = if is_p_neg {
				q1.checked_add(&q2)?
			} else {
				q1.checked_sub(&q2)?
			};

			let r = Increase(self.hub_state_in.hub_reserve_at_block_start)
				.checked_mul(&Increase(delta_hub_reserve_out_gross).checked_sub(&c_k)?)?;

			let q_hp = to_u256!(*q);
			let right_side = to_u256!(*(r.checked_mul(&Increase(4u128))?.checked_mul_fixed(p)?));
			let disc = if r.is_positive() && !is_p_neg || !r.is_positive() && is_p_neg {
				q_hp.checked_mul(q_hp)?.checked_sub(right_side)?
			} else {
				q_hp.checked_mul(q_hp)?.checked_add(right_side)?
			};
			let sd_hp = disc.integer_sqrt();
			let sd = to_balance!(sd_hp).ok()?;

			let u = if q >= Increase(0) {
				if r.is_positive() {
					Decrease((*r).checked_mul(2)?.checked_div((*q).checked_add(sd)?)?)
				} else {
					Increase((*r).checked_mul(2)?.checked_div((*q).checked_add(sd)?)?)
				}
			} else if r.is_positive() {
				Increase((*r).checked_mul(2)?.checked_div((*q).checked_add(sd)?)?)
			} else {
				Decrease((*r).checked_mul(2)?.checked_div((*q).checked_add(sd)?)?)
			};

			let delta_hub_reserve_in = *(self.hub_state_in.current_delta_hub_reserve.checked_sub(&u)?);
			Some(delta_hub_reserve_in)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::BalanceUpdate;
	use super::Ordering;
	use super::{CheckedAdd, CheckedSub};
	//use cool_asserts::assert_panics;
	use test_case::test_case;

	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(100), BalanceUpdate::Increase(200) ; "When both increase")]
	#[test_case(BalanceUpdate::Increase(500), BalanceUpdate::Decrease(300), BalanceUpdate::Increase(200) ; "When increase and decrease")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(300), BalanceUpdate::Decrease(200) ; "When increase and decrease larger")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(0), BalanceUpdate::Increase(100) ; "When increase and increase by zero")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(0), BalanceUpdate::Increase(100) ; "When increase and decrease by zero")]
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(100) ; "When increase zero and decrease")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(300), BalanceUpdate::Decrease(400) ; "When both decrease")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(100), BalanceUpdate::Decrease(100) ; "When decrease and increase")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(300), BalanceUpdate::Increase(100) ; "When decrease and increase larger")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(0), BalanceUpdate::Decrease(200) ; "When decrease and increase zero")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(100) ; "When decrease zero and decrease")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(100), BalanceUpdate::Increase(0) ; "When decrease and decrease same amount")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Increase(100), BalanceUpdate::Decrease(0) ; "When decrease and decrease same amount swapped")] // should be probably same as previous ?
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
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Decrease(100), Some(BalanceUpdate::Decrease(100)) ; "When increase zero and decrease")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(300), Some(BalanceUpdate::Decrease(400)) ; "When both decrease")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(100), Some(BalanceUpdate::Decrease(100)) ; "When decrease and increase")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(300), Some(BalanceUpdate::Increase(100)) ; "When decrease and increase larger")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(0), Some(BalanceUpdate::Decrease(200)) ; "When decrease and increase zero")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Decrease(100), Some(BalanceUpdate::Decrease(100)) ; "When decrease zero and decrease")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(100), Some(BalanceUpdate::Increase(0)) ; "When decrease and decrease same amount")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Increase(100), Some(BalanceUpdate::Decrease(0)) ; "When decrease and decrease same amount swapped")] // should be probably same as previous ?
	#[test_case(BalanceUpdate::Increase(u32::MAX), BalanceUpdate::Decrease(1), Some(BalanceUpdate::Increase(u32::MAX - 1)) ; "When increase max and decrease one")]
	#[test_case(BalanceUpdate::Increase(u32::MAX), BalanceUpdate::Increase(1), None ; "When increase overflows")]
	#[test_case(BalanceUpdate::Decrease(u32::MAX), BalanceUpdate::Decrease(1), None ; "When decrease overflows")]
	fn balance_update_checked_add(x: BalanceUpdate<u32>, y: BalanceUpdate<u32>, result: Option<BalanceUpdate<u32>>) {
		assert_eq!(x.checked_add(&y), result);
	}

	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(100), BalanceUpdate::Increase(0) ; "When both increase")]
	#[test_case(BalanceUpdate::Increase(300), BalanceUpdate::Increase(100), BalanceUpdate::Increase(200) ; "When increase larger and increase")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(300), BalanceUpdate::Decrease(200) ; "When increase and increase larger")]
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Increase(0), BalanceUpdate::Increase(0) ; "When both increase zero")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(0), BalanceUpdate::Increase(100) ; "When increase and increase by zero")]
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Increase(100), BalanceUpdate::Decrease(100) ; "When increase by zero and increase")]
	#[test_case(BalanceUpdate::Increase(u32::MAX), BalanceUpdate::Increase(1), BalanceUpdate::Increase(u32::MAX - 1) ; "When increase max and increase one")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(100), BalanceUpdate::Increase(0) ; "When both decrease")]
	#[test_case(BalanceUpdate::Decrease(300), BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(200) ; "When decrease larget and decrease")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(300), BalanceUpdate::Increase(200) ; "When decrease and decrease larger")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Decrease(100), BalanceUpdate::Increase(100) ; "When decrease zero and decrease")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(0), BalanceUpdate::Decrease(100) ; "When decrease and decrease zero")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Decrease(0), BalanceUpdate::Increase(0) ; "When both decrease zero")]
	#[test_case(BalanceUpdate::Decrease(u32::MAX), BalanceUpdate::Decrease(1), BalanceUpdate::Decrease(u32::MAX - 1) ; "When decrease max and decrease one")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(100), BalanceUpdate::Increase(200) ; "When increase and decrease same amount ")]
	#[test_case(BalanceUpdate::Increase(500), BalanceUpdate::Decrease(300), BalanceUpdate::Increase(800) ; "When increase larger and decrease")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(300), BalanceUpdate::Increase(400) ; "When increase and decrease larger")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(0), BalanceUpdate::Increase(100) ; "When increase and decrease by zero")]
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Decrease(100), BalanceUpdate::Increase(100) ; "When increase zero and decrease")]
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Decrease(0), BalanceUpdate::Increase(0) ; "When increase zero and decrease zero")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Increase(100), BalanceUpdate::Decrease(200) ; "When decrease and increase same amount")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(100), BalanceUpdate::Decrease(300) ; "When decrease larger and increase")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(300), BalanceUpdate::Decrease(500) ; "When decrease and increase larger")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(0), BalanceUpdate::Decrease(200) ; "When decrease and increase zero")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Increase(100), BalanceUpdate::Decrease(100) ; "When decrease zero and increase")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Increase(0), BalanceUpdate::Increase(0) ; "When decrease zero and increase zero")]
	fn balance_update_sub(x: BalanceUpdate<u32>, y: BalanceUpdate<u32>, result: BalanceUpdate<u32>) {
		assert_eq!(x - y, result);
	}

	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(100), Some(BalanceUpdate::Increase(0)) ; "When both increase")]
	#[test_case(BalanceUpdate::Increase(300), BalanceUpdate::Increase(100), Some(BalanceUpdate::Increase(200)) ; "When increase larger and increase")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(300), Some(BalanceUpdate::Decrease(200)) ; "When increase and increase larger")]
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Increase(0), Some(BalanceUpdate::Increase(0)) ; "When both increase zero")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(0), Some(BalanceUpdate::Increase(100)) ; "When increase and increase by zero")]
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Increase(100), Some(BalanceUpdate::Decrease(100)) ; "When increase by zero and increase")]
	#[test_case(BalanceUpdate::Increase(u32::MAX), BalanceUpdate::Increase(1), Some(BalanceUpdate::Increase(u32::MAX - 1)) ; "When increase max and increase one")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(100), Some(BalanceUpdate::Increase(0)) ; "When both decrease")]
	#[test_case(BalanceUpdate::Decrease(300), BalanceUpdate::Decrease(100), Some(BalanceUpdate::Decrease(200)) ; "When decrease larget and decrease")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(300), Some(BalanceUpdate::Increase(200)) ; "When decrease and decrease larger")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Decrease(100), Some(BalanceUpdate::Increase(100)) ; "When decrease zero and decrease")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(0), Some(BalanceUpdate::Decrease(100)) ; "When decrease and decrease zero")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Decrease(0), Some(BalanceUpdate::Increase(0)) ; "When both decrease zero")]
	#[test_case(BalanceUpdate::Decrease(u32::MAX), BalanceUpdate::Decrease(1), Some(BalanceUpdate::Decrease(u32::MAX - 1)) ; "When decrease max and decrease one")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(100), Some(BalanceUpdate::Increase(200)) ; "When increase and decrease same amount ")]
	#[test_case(BalanceUpdate::Increase(500), BalanceUpdate::Decrease(300), Some(BalanceUpdate::Increase(800)) ; "When increase larger and decrease")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(300), Some(BalanceUpdate::Increase(400)) ; "When increase and decrease larger")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(0), Some(BalanceUpdate::Increase(100)) ; "When increase and decrease by zero")]
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Decrease(100), Some(BalanceUpdate::Increase(100)) ; "When increase zero and decrease")]
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Decrease(0), Some(BalanceUpdate::Increase(0)) ; "When increase zero and decrease zero")]
	#[test_case(BalanceUpdate::Increase(u32::MAX), BalanceUpdate::Decrease(1), None ; "When increase max and decrease one")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Increase(100), Some(BalanceUpdate::Decrease(200)) ; "When decrease and increase same amount")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(100), Some(BalanceUpdate::Decrease(300)) ; "When decrease larger and increase")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(300), Some(BalanceUpdate::Decrease(500)) ; "When decrease and increase larger")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(0), Some(BalanceUpdate::Decrease(200)) ; "When decrease and increase zero")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Increase(100), Some(BalanceUpdate::Decrease(100)) ; "When decrease zero and increase")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Increase(0), Some(BalanceUpdate::Increase(0)) ; "When decrease zero and increase zero")]
	#[test_case(BalanceUpdate::Decrease(u32::MAX), BalanceUpdate::Increase(1), None ; "When decrease max and increase one")]
	fn balance_update_checked_sub(x: BalanceUpdate<u32>, y: BalanceUpdate<u32>, result: Option<BalanceUpdate<u32>>) {
		assert_eq!(x.checked_sub(&y), result);
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
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(100) ; "When decrease zero and decreases ")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(100), BalanceUpdate::Increase(0) ; "When decrease and decrease same amount ")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Increase(100), BalanceUpdate::Decrease(0) ; "When decrease and decrease same amount swapped ")]
	#[test_case(BalanceUpdate::Increase(u32::MAX), BalanceUpdate::Decrease(1), BalanceUpdate::Increase(u32::MAX - 1) ; "When increase max and decrease one")]
	#[test_case(BalanceUpdate::Increase(u32::MAX), BalanceUpdate::Increase(1), BalanceUpdate::Increase(u32::MAX); "When increase overflows")]
	#[test_case(BalanceUpdate::Decrease(u32::MAX), BalanceUpdate::Decrease(1), BalanceUpdate::Decrease(u32::MAX); "When decrease overflows")]
	fn balance_update_saturating_add(x: BalanceUpdate<u32>, y: BalanceUpdate<u32>, result: BalanceUpdate<u32>) {
		assert_eq!(x.saturating_merge(y), result);
	}

	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Increase(0), Ordering::Equal ; "When both increase zero")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(100), Ordering::Equal ; "When both increase")]
	#[test_case(BalanceUpdate::Increase(500), BalanceUpdate::Decrease(300), Ordering::Greater ; "When increase larger than decrease")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(300), Ordering::Greater ; "When increase and decrease larger")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(0), Ordering::Greater ; "When increase and increase by zero")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(0), Ordering::Greater ; "When increase and decrease by zero")]
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Decrease(100), Ordering::Greater ; "When increase zero and decrease ")]
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Decrease(0), Ordering::Equal ; "When increase zero and decrease zero")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(300), Ordering::Greater ; "When both decrease ")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(100), Ordering::Less ; "When decrease and increase")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(300), Ordering::Less ; "When decrease and increase larger")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(0), Ordering::Less ; "When decrease and increase zero")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Decrease(100), Ordering::Greater ; "When decrease zero and decreases ")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(100), Ordering::Greater ; "When decrease and decrease same amount ")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Increase(100), Ordering::Less ; "When decrease and decrease same amount swapped ")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Increase(0), Ordering::Equal ; "When decrease zero and increase zero")]
	fn balance_update_partial_ord(x: BalanceUpdate<u32>, y: BalanceUpdate<u32>, result: Ordering) {
		assert_eq!(x.partial_cmp(&y), Some(result));
	}

	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Increase(0), Ordering::Equal ; "When both increase zero")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(100), Ordering::Equal ; "When both increase")]
	#[test_case(BalanceUpdate::Increase(500), BalanceUpdate::Decrease(300), Ordering::Greater ; "When increase larger than decrease")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(300), Ordering::Greater ; "When increase and decrease larger")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Increase(0), Ordering::Greater ; "When increase and increase by zero")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(0), Ordering::Greater ; "When increase and decrease by zero")]
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Decrease(100), Ordering::Greater ; "When increase zero and decrease ")]
	#[test_case(BalanceUpdate::Increase(0), BalanceUpdate::Decrease(0), Ordering::Equal ; "When increase zero and decrease zero")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Decrease(300), Ordering::Greater ; "When both decrease ")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(100), Ordering::Less ; "When decrease and increase")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(300), Ordering::Less ; "When decrease and increase larger")]
	#[test_case(BalanceUpdate::Decrease(200), BalanceUpdate::Increase(0), Ordering::Less ; "When decrease and increase zero")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Decrease(100), Ordering::Greater ; "When decrease zero and decreases ")]
	#[test_case(BalanceUpdate::Increase(100), BalanceUpdate::Decrease(100), Ordering::Greater ; "When decrease and decrease same amount ")]
	#[test_case(BalanceUpdate::Decrease(100), BalanceUpdate::Increase(100), Ordering::Less ; "When decrease and decrease same amount swapped ")]
	#[test_case(BalanceUpdate::Decrease(0), BalanceUpdate::Increase(0), Ordering::Equal ; "When decrease zero and increase zero")]
	fn balance_update_ord(x: BalanceUpdate<u32>, y: BalanceUpdate<u32>, result: Ordering) {
		assert_eq!(x.cmp(&y), result);
	}
}
