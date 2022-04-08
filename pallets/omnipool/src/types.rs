use super::*;
use frame_support::pallet_prelude::*;
use sp_runtime::{FixedPointNumber, FixedU128};

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
	Balance: Into<<FixedU128 as FixedPointNumber>::Inner> + Clone,
{
	/// Calcuate price for actual state
	pub(super) fn price(&self) -> FixedU128 {
		FixedU128::from((self.hub_reserve.clone().into(), self.reserve.clone().into()))
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
	pub(super) price: Balance,
}

// Using FixedU128 to represent a price which uses u128 as inner type, so let's convert `Balance` into FixedU128
impl<Balance, AssetId> Position<Balance, AssetId>
where
	Balance: Clone + From<u128> + Into<u128>,
{
	pub(super) fn fixed_price(&self) -> Price {
		Price::from_inner(self.price.clone().into())
	}

	pub(super) fn price_to_balance(price: Price) -> Balance {
		price.into_inner().into()
	}
}

pub(super) enum ImbalanceUpdate<Balance> {
	Increase(Balance),
	Decrease(Balance),
}

/// Simple type to represent imbalance which can be positive or negative.
// Note: Simple prefix is used not to confuse with Imbalance trait from frame_support.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub(super) struct SimpleImbalance<Balance> {
	pub(super) value: Balance,
	pub(super) negative: bool,
}

impl<Balance: Default> Default for SimpleImbalance<Balance> {
	fn default() -> Self {
		Self {
			value: Balance::default(),
			negative: true,
		}
	}
}

impl<Balance: CheckedAdd + CheckedSub + PartialOrd> SimpleImbalance<Balance> {
	pub(super) fn add(mut self, amount: Balance) -> Option<Self> {
		if self.is_positive() {
			self.value = self.value.checked_add(&amount)?;
			Some(self)
		} else if self.value < amount {
			self.value = amount.checked_sub(&self.value)?;
			self.negative = false;
			Some(self)
		} else {
			self.value = self.value.checked_sub(&amount)?;
			Some(self)
		}
	}

	pub(super) fn sub(mut self, amount: Balance) -> Option<Self> {
		if self.is_negative() {
			self.value = self.value.checked_add(&amount)?;
			Some(self)
		} else if self.value < amount {
			self.value = amount.checked_sub(&self.value)?;
			self.negative = true;
			Some(self)
		} else {
			self.value = self.value.checked_sub(&amount)?;
			Some(self)
		}
	}

	pub(super) fn is_negative(&self) -> bool {
		self.negative
	}

	pub(super) fn is_positive(&self) -> bool {
		!self.negative
	}
}
