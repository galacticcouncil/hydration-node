use codec::{Decode, Encode, MaxEncodedLen};
use pallet_route_executor::Trade;
use scale_info::TypeInfo;
use sp_runtime::traits::ConstU32;
use sp_runtime::{BoundedVec, Permill};

pub type Balance = u128;
pub type ScheduleId = u32;
pub type NamedReserveIdentifier = [u8; 8];

const MAX_NUMBER_OF_TRADES: u32 = 5;

#[derive(Encode, Decode, Debug, Eq, PartialEq, Clone, TypeInfo, MaxEncodedLen)]
pub struct Schedule<AccountId, AssetId, BlockNumber> {
	pub owner: AccountId,
	pub period: BlockNumber,
	pub total_amount: Balance,
	pub max_retries: Option<u8>,
	pub stability_threshold: Option<Permill>,
	pub slippage: Option<Permill>,
	pub order: Order<AssetId>,
}

#[derive(Encode, Decode, Debug, Eq, PartialEq, Clone, TypeInfo, MaxEncodedLen)]
pub enum Order<AssetId> {
	Sell {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_limit: Balance,
		route: BoundedVec<Trade<AssetId>, ConstU32<MAX_NUMBER_OF_TRADES>>,
	},
	Buy {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_limit: Balance,
		route: BoundedVec<Trade<AssetId>, ConstU32<MAX_NUMBER_OF_TRADES>>,
	},
}

impl<AssetId> Order<AssetId>
where
	AssetId: Copy,
{
	pub fn get_asset_in(&self) -> AssetId {
		let asset_in = match &self {
			Order::Sell { asset_in, .. } => asset_in,
			Order::Buy { asset_in, .. } => asset_in,
		};
		*asset_in
	}

	pub fn get_asset_out(&self) -> AssetId {
		let asset_out = match &self {
			Order::Sell { asset_out, .. } => asset_out,
			Order::Buy { asset_out, .. } => asset_out,
		};
		*asset_out
	}

	pub fn get_route_length(&self) -> usize {
		let route = match &self {
			Order::Sell { route, .. } => route,
			Order::Buy { route, .. } => route,
		};

		route.len()
	}
}
