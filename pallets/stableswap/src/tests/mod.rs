use crate::tests::mock::*;
use crate::*;
use sp_runtime::FixedU128;

mod add_liquidity;
mod amplification;
mod creation;
mod hooks;
mod invariants;
pub(crate) mod mock;
mod price;
mod remove_liquidity;
mod trades;
mod update_pool;

type Balance = u128;

#[macro_export]
macro_rules! to_precision {
	($e:expr, $f:expr) => {
		$e * 10u128.pow($f as u32)
	};
}

pub(crate) fn get_share_price(pool_id: AssetId, asset_idx: usize) -> FixedU128 {
	let pool_account = pool_account(pool_id);
	let pool = <Pools<Test>>::get(pool_id).unwrap();
	let balances = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
	let amp = Pallet::<Test>::get_amplification(&pool);
	let issuance = Tokens::total_issuance(pool_id);
	let share_price =
		hydra_dx_math::stableswap::calculate_share_price::<128u8>(&balances, amp, issuance, asset_idx, None).unwrap();
	FixedU128::from_rational(share_price.0, share_price.1)
}

pub(crate) fn asset_spot_price(pool_id: AssetId, asset_id: AssetId) -> FixedU128 {
	let pool_account = pool_account(pool_id);
	let pool = <Pools<Test>>::get(pool_id).unwrap();
	let balances = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
	let amp = Pallet::<Test>::get_amplification(&pool);
	let asset_idx = pool.find_asset(asset_id).unwrap();
	let d = hydra_dx_math::stableswap::calculate_d::<D_ITERATIONS>(&balances, amp).unwrap();
	let p = hydra_dx_math::stableswap::calculate_spot_price(&balances, amp, d, asset_idx).unwrap();
	FixedU128::from_rational(p.0, p.1)
}
