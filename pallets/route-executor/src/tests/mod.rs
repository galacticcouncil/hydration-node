use crate::tests::mock::*;
use hydradx_traits::router::Trade;
use sp_runtime::traits::ConstU32;
use sp_runtime::BoundedVec;

mod buy;
pub mod mock;
pub mod sell;
pub mod set_route;

pub fn create_bounded_vec(trades: Vec<Trade<AssetId>>) -> BoundedVec<Trade<AssetId>, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade<AssetId>, sp_runtime::traits::ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
}
