use crate::tests::mock::*;
use crate::{Balance, Order, Recurrence, Schedule, ScheduleId, Trade};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::traits::ConstU32;
use sp_runtime::BoundedVec;
use sp_runtime::Permill;

pub mod mock;
pub mod on_initialize;
mod pause;
pub mod resume;
pub mod schedule;
pub mod terminate;

#[macro_export]
macro_rules! assert_balance {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(Currencies::free_balance($y, &$x), $z);
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

	fn build(self) -> Schedule<AssetId, BlockNumber> {
		Schedule {
			period: self.period.unwrap(),
			recurrence: self.recurrence.unwrap(),
			order: self.order.unwrap(),
		}
	}
}
pub fn empty_vec() -> BoundedVec<Trade<AssetId>, ConstU32<5>> {
	create_bounded_vec(vec![])
}

pub fn create_bounded_vec(trades: Vec<Trade<AssetId>>) -> BoundedVec<Trade<AssetId>, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade<AssetId>, sp_runtime::traits::ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
}

#[macro_export]
macro_rules! assert_scheduled_ids {
	($block:expr, $expected_schedule_ids:expr) => {
		let actual_schedule_ids = DCA::schedule_ids_per_block($block);
		assert!(DCA::schedule_ids_per_block($block).is_some());
		let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids($expected_schedule_ids);
		assert_eq!(actual_schedule_ids.unwrap(), expected_scheduled_ids_for_next_block);
	};
}

fn create_bounded_vec_with_schedule_ids(schedule_ids: Vec<ScheduleId>) -> BoundedVec<ScheduleId, ConstU32<5>> {
	let bounded_vec: BoundedVec<ScheduleId, sp_runtime::traits::ConstU32<5>> = schedule_ids.try_into().unwrap();
	bounded_vec
}

pub fn set_storage_bond_config(amount: Balance) {
	STORAGE_BOND.with(|v| {
		*v.borrow_mut() = amount;
	});
}

pub fn set_slippage_config(percentage: Permill) {
	SLIPPAGE.with(|v| {
		*v.borrow_mut() = percentage;
	});
}

pub fn set_execution_bond_config(amount: Balance) {
	EXECUTION_BOND.with(|v| {
		*v.borrow_mut() = amount;
	});
}
