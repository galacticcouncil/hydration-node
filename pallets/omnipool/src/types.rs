#![allow(clippy::bad_bit_mask)]

use super::*;
use codec::MaxEncodedLen;
use frame_support::pallet_prelude::*;
use hydra_dx_math::omnipool::types::{AssetReserveState as MathReserveState, AssetStateChange, BalanceUpdate};
use sp_runtime::{FixedPointNumber, FixedU128};

/// Balance type used in Omnipool
pub type Balance = u128;

/// Fixed Balance type to represent asset price
pub type Price = FixedU128;

bitflags::bitflags! {
	/// Indicates whether asset can be bought or sold to/from Omnipool and/or liquidity added/removed.
	#[derive(Encode,Decode, MaxEncodedLen, TypeInfo)]
	pub struct Tradability: u8 {
		/// Asset is frozen. No operations are allowed.
		const FROZEN = 0b0000_0000;
		/// Asset is allowed to be sold into omnipool
		const SELL = 0b0000_0001;
		/// Asset is allowed to be bought into omnipool
		const BUY = 0b0000_0010;
		/// Adding liquidity of asset is allowed
		const ADD_LIQUIDITY = 0b0000_0100;
		/// Removing liquidity of asset is not allowed
		const REMOVE_LIQUIDITY = 0b0000_1000;
	}
}

impl Default for Tradability {
	fn default() -> Self {
		Tradability::SELL | Tradability::BUY | Tradability::ADD_LIQUIDITY | Tradability::REMOVE_LIQUIDITY
	}
}

impl Tradability {
	pub(crate) fn is_safe_withdrawal(&self) -> bool {
		*self == Tradability::ADD_LIQUIDITY | Tradability::REMOVE_LIQUIDITY || *self == Tradability::REMOVE_LIQUIDITY
	}
}

#[derive(Clone, Default, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct AssetState<Balance> {
	/// Quantity of Hub Asset matching this asset
	pub(super) hub_reserve: Balance,
	/// Quantity of LP shares for this asset
	pub(super) shares: Balance,
	/// Quantity of LP shares for this asset owned by protocol
	pub(super) protocol_shares: Balance,
	/// Asset's weight cap
	/// Note: this should be Permill or FixedU128. But neither implements MaxEncodedLen in 0.9.16.
	/// TODO: upgrade to 0.9.17 resolves this.
	pub cap: u128,
	/// Asset's trade state
	pub tradable: Tradability,
}

impl<Balance> From<AssetReserveState<Balance>> for AssetState<Balance>
where
	Balance: Copy,
{
	fn from(s: AssetReserveState<Balance>) -> Self {
		Self {
			hub_reserve: s.hub_reserve,
			shares: s.shares,
			protocol_shares: s.protocol_shares,
			cap: s.cap,
			tradable: s.tradable,
		}
	}
}

impl<Balance> From<(MathReserveState<Balance>, Permill, Tradability)> for AssetState<Balance>
where
	Balance: Copy,
{
	fn from((state, cap, tradable): (MathReserveState<Balance>, Permill, Tradability)) -> Self {
		Self {
			hub_reserve: state.hub_reserve,
			shares: state.shares,
			protocol_shares: state.protocol_shares,
			cap: FixedU128::from(cap).into_inner(),
			tradable,
		}
	}
}

/// Position in Omnipool represents a moment when LP provided liquidity of an asset at that moment’s price.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Position<Balance, AssetId> {
	/// Provided Asset
	pub asset_id: AssetId,
	/// Amount of asset added to omnipool
	pub amount: Balance,
	/// Quantity of LP shares owned by LP
	pub shares: Balance,
	/// Price at which liquidity was provided - ( hub reserve, asset reserve ) at the time of creation
	pub price: (Balance, Balance),
}

impl<Balance, AssetId> From<&Position<Balance, AssetId>> for hydra_dx_math::omnipool::types::Position<Balance>
where
	Balance: Copy + Into<u128>,
{
	fn from(position: &Position<Balance, AssetId>) -> Self {
		Self {
			amount: position.amount,
			shares: position.shares,
			price: position.price,
		}
	}
}

impl<Balance, AssetId> Position<Balance, AssetId>
where
	Balance: Into<<FixedU128 as FixedPointNumber>::Inner> + Copy + CheckedAdd + CheckedSub + Default,
{
	pub(super) fn price_from_rational(&self) -> Option<FixedU128> {
		FixedU128::checked_from_rational(self.price.0.into(), self.price.1.into())
	}

	/// Update current position state with given delta changes.
	pub(super) fn delta_update(
		self,
		delta_reserve: &BalanceUpdate<Balance>,
		delta_shares: &BalanceUpdate<Balance>,
	) -> Option<Self> {
		Some(Self {
			asset_id: self.asset_id,
			amount: (*delta_reserve + self.amount)?,
			shares: (*delta_shares + self.shares)?,
			price: self.price,
		})
	}
}

/// Asset state representation including asset pool reserve.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct AssetReserveState<Balance> {
	/// Quantity of asset in omnipool
	pub reserve: Balance,
	/// Quantity of Hub Asset matching this asset
	pub hub_reserve: Balance,
	/// Quantity of LP shares for this asset
	pub shares: Balance,
	/// Quantity of LP shares for this asset owned by protocol
	pub protocol_shares: Balance,
	/// Asset's weight cap
	pub cap: u128,
	/// Asset's trade state
	pub tradable: Tradability,
}

impl<Balance> From<&AssetReserveState<Balance>> for MathReserveState<Balance>
where
	Balance: Copy,
{
	fn from(state: &AssetReserveState<Balance>) -> Self {
		Self {
			reserve: state.reserve,
			hub_reserve: state.hub_reserve,
			shares: state.shares,
			protocol_shares: state.protocol_shares,
		}
	}
}

impl<Balance> From<AssetReserveState<Balance>> for MathReserveState<Balance>
where
	Balance: Copy,
{
	fn from(state: AssetReserveState<Balance>) -> Self {
		Self {
			reserve: state.reserve,
			hub_reserve: state.hub_reserve,
			shares: state.shares,
			protocol_shares: state.protocol_shares,
		}
	}
}

impl<Balance> From<(&AssetState<Balance>, Balance)> for AssetReserveState<Balance>
where
	Balance: Copy,
{
	fn from((s, reserve): (&AssetState<Balance>, Balance)) -> Self {
		Self {
			reserve,
			hub_reserve: s.hub_reserve,
			shares: s.shares,
			protocol_shares: s.protocol_shares,
			cap: s.cap,
			tradable: s.tradable,
		}
	}
}

impl<Balance> From<(AssetState<Balance>, Balance)> for AssetReserveState<Balance>
where
	Balance: Copy,
{
	fn from((s, reserve): (AssetState<Balance>, Balance)) -> Self {
		Self {
			reserve,
			hub_reserve: s.hub_reserve,
			shares: s.shares,
			protocol_shares: s.protocol_shares,
			cap: s.cap,
			tradable: s.tradable,
		}
	}
}

impl<Balance> AssetReserveState<Balance>
where
	Balance: Into<<FixedU128 as FixedPointNumber>::Inner> + Copy + CheckedAdd + CheckedSub + Default,
{
	pub fn price_as_rational(&self) -> (Balance, Balance) {
		(self.hub_reserve, self.reserve)
	}

	/// Calculate price for actual state
	pub fn price(&self) -> Option<FixedU128> {
		FixedU128::checked_from_rational(self.hub_reserve.into(), self.reserve.into())
	}

	pub(crate) fn weight_cap(&self) -> FixedU128 {
		FixedU128::from_inner(self.cap)
	}

	/// Update current asset state with given delta changes.
	pub fn delta_update(self, delta: &AssetStateChange<Balance>) -> Option<Self> {
		Some(Self {
			reserve: (delta.delta_reserve + self.reserve)?,
			hub_reserve: (delta.delta_hub_reserve + self.hub_reserve)?,
			shares: (delta.delta_shares + self.shares)?,
			protocol_shares: (delta.delta_protocol_shares + self.protocol_shares)?,
			cap: self.cap,
			tradable: self.tradable,
		})
	}
}
