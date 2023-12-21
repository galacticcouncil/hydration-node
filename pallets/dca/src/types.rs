use codec::{Decode, Encode, MaxEncodedLen};
use hydradx_traits::router::{AssetPair, RouteProvider, Trade};
use scale_info::TypeInfo;
use sp_runtime::traits::ConstU32;
use sp_runtime::{BoundedVec, Permill};
use sp_std::vec::Vec;

pub type Balance = u128;
pub type ScheduleId = u32;
pub type NamedReserveIdentifier = [u8; 8];

const MAX_NUMBER_OF_TRADES: u32 = 5;

/// DCA schedule containing information to execute repeating orders.
#[derive(Encode, Decode, Debug, Eq, PartialEq, Clone, TypeInfo, MaxEncodedLen)]
pub struct Schedule<AccountId, AssetId, BlockNumber> {
	/// The owner of the schedule.
	pub owner: AccountId,
	/// The time period (in blocks) between two schedule executions.
	pub period: BlockNumber,
	/// The total amount (budget) the user wants to spend on the whole DCA.
	/// Its currency is the sold (amount_in) currency specified in `order`.
	pub total_amount: Balance,
	/// The maximum number of retries in case of failing schedules.
	/// If not specified, the default pallet configuration `MaxPriceDifferenceBetweenBlocks` is used.
	pub max_retries: Option<u8>,
	/// The price stability threshold used to check if the price is stable.
	/// The check is performed by comparing the spot price and short oracle price.
	/// If not specified, the default pallet configuration `MaxPriceDifferenceBetweenBlocks` is used.
	pub stability_threshold: Option<Permill>,
	/// The slippage limit used to calculate the `min_amount_out` and `max_amount_in` trade limits.
	pub slippage: Option<Permill>,
	/// The order containing information to execute a specific trade by the router.
	pub order: Order<AssetId>,
}

#[derive(Encode, Decode, Debug, Eq, PartialEq, Clone, TypeInfo, MaxEncodedLen)]
pub enum Order<AssetId> {
	Sell {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_amount_out: Balance,
		route: BoundedVec<Trade<AssetId>, ConstU32<MAX_NUMBER_OF_TRADES>>,
	},
	Buy {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_amount_in: Balance,
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

	pub fn get_route_or_default<Provider: RouteProvider<AssetId>>(&self) -> Vec<Trade<AssetId>> {
		let route = match &self {
			Order::Sell { route, .. } => route,
			Order::Buy { route, .. } => route,
		};
		if route.is_empty() {
			Provider::get_route(AssetPair::new(self.get_asset_in(), self.get_asset_out()))
		} else {
			route.to_vec()
		}
	}
}
