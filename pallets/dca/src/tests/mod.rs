use crate::tests::mock::*;
use crate::{
	Balance, Event, Order, RemainingAmounts, RetriesOnError, Schedule, ScheduleExecutionBlock, ScheduleId,
	ScheduleIdSequencer, ScheduleIdsPerBlock, ScheduleOwnership, Schedules,
};
use frame_support::assert_ok;
use hydradx_traits::router::PoolType;
use hydradx_traits::router::RouterT;
use hydradx_traits::router::Trade;
use orml_traits::NamedMultiReservableCurrency;
use sp_runtime::traits::ConstU32;
use sp_runtime::{BoundedVec, Permill};

pub mod migration;
pub mod mock;
pub mod on_initialize;
pub mod schedule;
pub mod storage_injection_fidelity;
pub mod terminate;
pub mod unlock_reserves;

#[macro_export]
macro_rules! assert_balance {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(Currencies::free_balance($y, &$x), $z);
	}};
}

pub struct ScheduleBuilder {
	pub owner: Option<AccountId>,
	pub period: Option<BlockNumber>,
	pub order: Option<Order<AssetId>>,
	pub total_amount: Option<Balance>,
	pub max_retries: Option<Option<u8>>,
	pub slippage: Option<Option<Permill>>,
	pub stability_threshold: Option<Option<Permill>>,
}

impl ScheduleBuilder {
	pub fn new() -> ScheduleBuilder {
		ScheduleBuilder {
			owner: Some(ALICE),
			period: Some(ONE_HUNDRED_BLOCKS),
			stability_threshold: Some(None),
			slippage: Some(None),
			total_amount: Some(1000 * ONE),
			max_retries: Some(None),
			order: Some(Order::Sell {
				asset_in: HDX,
				asset_out: BTC,
				amount_in: 10 * ONE,
				min_amount_out: 0,
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

	pub fn build(self) -> Schedule<AccountId, AssetId, BlockNumber> {
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

/// Stores a schedule by writing exactly the state the `schedule` extrinsic writes.
///
/// Buy orders can no longer be scheduled, but buy schedules stored before that restriction keep
/// executing, so their execution still needs coverage. Fidelity against the extrinsic is pinned by
/// the tests in `storage_injection_fidelity`.
pub fn insert_schedule_into_storage(
	who: AccountId,
	schedule: Schedule<AccountId, AssetId, BlockNumber>,
	start_execution_block: Option<BlockNumber>,
) -> ScheduleId {
	assert_eq!(schedule.owner, who, "owner must match the schedule owner");

	let asset_in = schedule.order.get_asset_in();
	let route = schedule.order.get_route_or_default::<DefaultRouteProvider>();
	let amount_in = match schedule.order {
		Order::Sell { amount_in, .. } => amount_in,
		Order::Buy { amount_out, .. } => {
			RouteExecutor::calculate_buy_trade_amounts(&route, amount_out)
				.unwrap()
				.last()
				.unwrap()
				.amount_in
		}
	};
	let transaction_fee = DCA::get_transaction_fee(&schedule.order, None).unwrap();

	let reserve_amount = if schedule.is_rolling() {
		amount_in.saturating_add(transaction_fee).saturating_mul(2)
	} else {
		schedule.total_amount
	};

	let schedule_id = ScheduleIdSequencer::<Test>::mutate(|current_id| {
		let schedule_id = *current_id;
		*current_id += 1;
		schedule_id
	});

	Schedules::<Test>::insert(schedule_id, &schedule);
	ScheduleOwnership::<Test>::insert(who, schedule_id, ());
	RemainingAmounts::<Test>::insert(schedule_id, reserve_amount);
	RetriesOnError::<Test>::insert(schedule_id, 0);

	assert_ok!(Currencies::reserve_named(
		&NamedReserveId::get(),
		asset_in,
		&who,
		reserve_amount
	));

	let execution_block = first_execution_block(start_execution_block);
	ScheduleIdsPerBlock::<Test>::mutate(execution_block, |schedule_ids| {
		schedule_ids.try_push(schedule_id).expect("execution block is full");
	});
	ScheduleExecutionBlock::<Test>::insert(schedule_id, execution_block);

	System::deposit_event(RuntimeEvent::DCA(Event::Scheduled {
		id: schedule_id,
		who,
		period: schedule.period,
		total_amount: schedule.total_amount,
		order: schedule.order,
	}));
	System::deposit_event(RuntimeEvent::DCA(Event::ExecutionPlanned {
		id: schedule_id,
		who,
		block: execution_block,
	}));

	schedule_id
}

/// Mirrors `crate::Pallet::get_first_execution_block`.
fn first_execution_block(start_execution_block: Option<BlockNumber>) -> BlockNumber {
	let next_block = System::block_number().saturating_add(2);
	match start_execution_block {
		Some(block) => {
			let number = next_block.max(block);
			match number % 5 {
				0 => number,
				remainder => number.saturating_add(5 - remainder),
			}
		}
		None => next_block,
	}
}

pub fn create_bounded_vec(trades: Vec<Trade<AssetId>>) -> BoundedVec<Trade<AssetId>, ConstU32<9>> {
	let bounded_vec: BoundedVec<Trade<AssetId>, sp_runtime::traits::ConstU32<9>> = trades.try_into().unwrap();
	bounded_vec
}

pub fn create_bounded_vec_with_schedule_ids(schedule_ids: Vec<ScheduleId>) -> BoundedVec<ScheduleId, ConstU32<9>> {
	let bounded_vec: BoundedVec<ScheduleId, sp_runtime::traits::ConstU32<9>> = schedule_ids.try_into().unwrap();
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
		assert!(DCA::schedule_execution_block($schedule_id).is_none());
		assert_eq!(DCA::retries_on_error($schedule_id), 0);
	};
}
