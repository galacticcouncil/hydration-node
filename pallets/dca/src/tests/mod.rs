use crate::tests::mock::*;
use crate::{AssetId, Balance, BlockNumber, Order, Recurrence, Schedule, Trade};
use sp_runtime::traits::ConstU32;
use sp_runtime::BoundedVec;

pub mod mock;
pub mod on_initialize;
mod pause;
pub mod schedule;

#[macro_export]
macro_rules! assert_balance {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(Tokens::free_balance($y, &$x), $z);
	}};
}

struct ScheduleBuilder {
	pub period: Option<BlockNumber>,
	pub order_asset_in: Option<AssetId>,
	pub order_asset_out: Option<AssetId>,
	pub order_amount_in: Option<Balance>,
	pub order_amount_out: Option<Balance>,
	pub order_limit: Option<Balance>,
	pub recurrence: Option<Recurrence>,
}

impl ScheduleBuilder {
	fn new() -> ScheduleBuilder {
		ScheduleBuilder {
			period: Some(ONE_HUNDRED_BLOCKS),
			recurrence: Some(Recurrence::Fixed(5)),
			order_asset_in: Some(DAI),
			order_asset_out: Some(BTC),
			order_amount_in: Some(ONE),
			order_amount_out: Some(ONE),
			order_limit: Some(crate::types::Balance::MAX),
		}
	}

	fn with_period(mut self, period: BlockNumber) -> ScheduleBuilder {
		self.period = Some(period);
		return self;
	}

	fn with_asset_in(mut self, asset_in: AssetId) -> ScheduleBuilder {
		self.order_asset_in = Some(asset_in);
		return self;
	}

	fn with_asset_out(mut self, asset_out: AssetId) -> ScheduleBuilder {
		self.order_asset_out = Some(asset_out);
		return self;
	}

	fn with_amount_in(mut self, amount: Balance) -> ScheduleBuilder {
		self.order_amount_in = Some(amount);
		return self;
	}

	fn with_amount_out(mut self, amount: Balance) -> ScheduleBuilder {
		self.order_amount_out = Some(amount);
		return self;
	}

	fn with_limit(mut self, limit: Balance) -> ScheduleBuilder {
		self.order_limit = Some(limit);
		return self;
	}

	fn with_recurrence(mut self, recurrence: Recurrence) -> ScheduleBuilder {
		self.recurrence = Some(recurrence);
		return self;
	}

	fn build(self) -> Schedule<AssetId> {
		Schedule {
			period: self.period.unwrap(),
			recurrence: self.recurrence.unwrap(),
			order: Order {
				asset_in: self.order_asset_in.unwrap(),
				asset_out: self.order_asset_out.unwrap(),
				amount_in: self.order_amount_in.unwrap(),
				amount_out: self.order_amount_out.unwrap(),
				limit: self.order_limit.unwrap(),
				route: create_bounded_vec(vec![]),
			},
		}
	}
}

fn schedule_fake(
	period: BlockNumber,
	asset_pair: AssetPair,
	amount: crate::types::Balance,
	recurrence: Recurrence,
) -> Schedule<AssetId> {
	let trades = create_bounded_vec(vec![]);

	let schedule = Schedule {
		period: period,
		order: Order {
			asset_in: asset_pair.asset_in,
			asset_out: asset_pair.asset_out,
			amount_in: amount,
			amount_out: amount,
			limit: crate::types::Balance::MAX,
			route: trades,
		},
		recurrence: recurrence,
	};
	schedule
}

pub struct AssetPair {
	asset_in: AssetId,
	asset_out: AssetId,
}
pub fn create_bounded_vec(trades: Vec<Trade>) -> BoundedVec<Trade, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade, sp_runtime::traits::ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
}
