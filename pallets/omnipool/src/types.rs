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

/// Position representation
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct PositionId<InstanceId>(pub InstanceId);

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
	#[allow(unused)]
	pub(super) fn fixed_price(&self) -> Price {
		Price::from_inner(self.price.clone().into())
	}

	#[allow(unused)]
	pub(super) fn price_to_balance(price: Price) -> Balance {
		price.into_inner().into()
	}
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
			negative: false,
		}
	}
}

impl<Balance> SimpleImbalance<Balance> {
	pub(super) fn add<T: Config>(&mut self, _amount: Balance) -> Result<(), DispatchError> {
		Ok(())
	}

	#[allow(unused)]
	pub(super) fn sub<T: Config>(&mut self, _amount: Balance) -> Result<(), DispatchError> {
		Ok(())
	}
}
