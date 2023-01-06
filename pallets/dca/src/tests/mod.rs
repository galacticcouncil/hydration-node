use crate::tests::mock::*;
use crate::{AssetId, Balance, BlockNumber, Order, Recurrence, Schedule, Trade};
use sp_runtime::traits::ConstU32;
use sp_runtime::BoundedVec;

pub mod mock;
pub mod on_initialize;
mod pause;
pub mod resume;
pub mod schedule;
pub mod terminate;

#[macro_export]
macro_rules! assert_balance {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(Tokens::free_balance($y, &$x), $z);
	}};
}

struct ScheduleBuilder {
	pub period: Option<BlockNumber>,
	pub order: Option<Order<AssetId>>,
	pub recurrence: Option<Recurrence>,
}

impl ScheduleBuilder {
	fn new() -> ScheduleBuilder {
		ScheduleBuilder {
			period: Some(ONE_HUNDRED_BLOCKS),
			recurrence: Some(Recurrence::Fixed(5)),
			order: Some(Order::Buy {
				asset_in: HDX,
				asset_out: BTC,
				amount_out: ONE,
				max_limit: Balance::MAX,
				route: create_bounded_vec(vec![]),
			}),
		}
	}

	fn with_period(mut self, period: BlockNumber) -> ScheduleBuilder {
		self.period = Some(period);
		return self;
	}

	fn with_order(mut self, buy_order: Order<AssetId>) -> ScheduleBuilder {
		self.order = Some(buy_order);
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
			order: self.order.unwrap(),
		}
	}
}
pub fn empty_vec() -> BoundedVec<Trade, ConstU32<5>> {
	create_bounded_vec(vec![])
}

pub fn create_bounded_vec(trades: Vec<Trade>) -> BoundedVec<Trade, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade, sp_runtime::traits::ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
}

//TODO:
/*
## How to handle error in on_initialize:
-using default schedule
(- surely wrap it transactional block - execue_schedules - should be transactional)

- Frido Martin had IncosistentState, might be fuine here too

- Use defensive_ok_or - THIS ONE LOOKS PROMISING


## how extrinsics are executed in the blocks

on_initialize - add ordering - we need to make sure in the runtime config that we put pallets before and after it
*/
