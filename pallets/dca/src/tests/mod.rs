use crate::tests::mock::*;
use crate::{Balance, Order, Schedule, ScheduleId};
use hydradx_traits::router::PoolType;
use hydradx_traits::router::Trade;
use sp_runtime::traits::ConstU32;
use sp_runtime::{BoundedVec, Permill};

pub mod mock;
pub mod on_initialize;
pub mod schedule;
pub mod terminate;

#[macro_export]
macro_rules! assert_balance {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(Currencies::free_balance($y, &$x), $z);
	}};
}

struct ScheduleBuilder {
	pub owner: Option<AccountId>,
	pub period: Option<BlockNumber>,
	pub order: Option<Order<AssetId>>,
	pub total_amount: Option<Balance>,
	pub max_retries: Option<Option<u8>>,
	pub slippage: Option<Option<Permill>>,
	pub stability_threshold: Option<Option<Permill>>,
}

impl ScheduleBuilder {
	fn new() -> ScheduleBuilder {
		ScheduleBuilder {
			owner: Some(ALICE),
			period: Some(ONE_HUNDRED_BLOCKS),
			stability_threshold: Some(None),
			slippage: Some(None),
			total_amount: Some(1000 * ONE),
			max_retries: Some(None),
			order: Some(Order::Buy {
				asset_in: HDX,
				asset_out: BTC,
				amount_out: ONE,
				max_amount_in: 2 * ONE,
				route: create_bounded_vec(vec![Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: BTC,
				}]),
			}),
		}
	}

	fn with_owner(mut self, owner: AccountId) -> ScheduleBuilder {
		self.owner = Some(owner);
		self
	}

	fn with_period(mut self, period: BlockNumber) -> ScheduleBuilder {
		self.period = Some(period);
		self
	}

	fn with_order(mut self, buy_order: Order<AssetId>) -> ScheduleBuilder {
		self.order = Some(buy_order);
		self
	}

	fn with_total_amount(mut self, total_amount: Balance) -> ScheduleBuilder {
		self.total_amount = Some(total_amount);
		self
	}

	fn with_price_stability_threshold(mut self, treshold: Option<Permill>) -> ScheduleBuilder {
		self.stability_threshold = Some(treshold);
		self
	}

	fn with_slippage(mut self, slippage: Option<Permill>) -> ScheduleBuilder {
		self.slippage = Some(slippage);
		self
	}

	fn with_max_retries(mut self, max_retries: Option<u8>) -> ScheduleBuilder {
		self.max_retries = Some(max_retries);
		self
	}

	fn build(self) -> Schedule<AccountId, AssetId, BlockNumber> {
		Schedule {
			owner: self.owner.unwrap(),
			period: self.period.unwrap(),
			stability_threshold: self.stability_threshold.unwrap(),
			slippage: self.slippage.unwrap(),
			total_amount: self.total_amount.unwrap(),
			max_retries: self.max_retries.unwrap(),
			order: self.order.unwrap(),
		}
	}
}

pub fn create_bounded_vec(trades: Vec<Trade<AssetId>>) -> BoundedVec<Trade<AssetId>, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade<AssetId>, sp_runtime::traits::ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
}

pub fn create_bounded_vec_with_schedule_ids(schedule_ids: Vec<ScheduleId>) -> BoundedVec<ScheduleId, ConstU32<5>> {
	let bounded_vec: BoundedVec<ScheduleId, sp_runtime::traits::ConstU32<5>> = schedule_ids.try_into().unwrap();
	bounded_vec
}
#[macro_export]
macro_rules! assert_scheduled_ids {
	($block:expr, $expected_schedule_ids:expr) => {
		let actual_schedule_ids = DCA::schedule_ids_per_block($block);
		assert!(!DCA::schedule_ids_per_block($block).is_empty());
		let expected_scheduled_ids_for_next_block = create_bounded_vec_with_schedule_ids($expected_schedule_ids);
		assert_eq!(actual_schedule_ids, expected_scheduled_ids_for_next_block);
	};
}

#[macro_export]
macro_rules! assert_that_schedule_has_been_removed_from_storages {
	($owner:expr,$schedule_id:expr) => {
		assert!(DCA::schedules($schedule_id).is_none());
		assert!(DCA::owner_of($owner, $schedule_id).is_none());
		assert!(DCA::remaining_amounts($schedule_id).is_none());
		assert_eq!(DCA::retries_on_error($schedule_id), 0);
	};
}
