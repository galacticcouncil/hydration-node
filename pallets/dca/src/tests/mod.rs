use crate::{AssetId, BlockNumber, Order, Recurrence, Schedule, Trade};
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

pub fn create_bounded_vec(trades: Vec<Trade>) -> BoundedVec<Trade, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade, sp_runtime::traits::ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
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
