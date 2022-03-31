use crate::mock::*;
use crate::types::{AssetState, Position, SimpleImbalance};
use crate::*;
use frame_support::assert_ok;
use sp_runtime::{FixedPointNumber, FixedU128};

mod add_liquidity;
mod add_token;
mod buy;
mod remove_liquidity;
mod sell;

const ONE: Balance = 1_000_000_000_000;
const LP1: u64 = 1;
const LP2: u64 = 2;
const LP3: u64 = 3;

const NATIVE_AMOUNT: Balance = 10_000 * ONE;

#[macro_export]
macro_rules! check_balance {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(Tokens::free_balance($y, &$x), $z);
	}};
}

#[macro_export]
macro_rules! check_state {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(HubAssetLiquidity::<Test>::get(), $x, "Hub liquidity incorrect");
		assert_eq!(TotalTVL::<Test>::get(), $y, "Total tvl incorrect");
		assert_eq!(HubAssetImbalance::<Test>::get(), $z, "Imbalance incorrect");
	}};
}

#[macro_export]
macro_rules! check_asset_state {
	( $x:expr, $y:expr) => {{
		let actual = Assets::<Test>::get($x).unwrap();
		assert_eq!(actual, $y);
	}};
}

fn init_omnipool(dai_amount: Balance, price: FixedU128) {
	assert_ok!(Omnipool::add_token(Origin::root(), DAI, dai_amount, price));
	assert_ok!(Omnipool::add_token(
		Origin::root(),
		HDX,
		NATIVE_AMOUNT,
		FixedU128::from(1)
	));

	check_state!(
		price.checked_mul_int(dai_amount).unwrap() + NATIVE_AMOUNT,
		NATIVE_AMOUNT * (dai_amount / price.checked_mul_int(dai_amount).unwrap()) + dai_amount,
		SimpleImbalance::default()
	);
}
