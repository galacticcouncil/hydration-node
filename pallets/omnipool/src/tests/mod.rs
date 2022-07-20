use crate::types::{Position, SimpleImbalance};
use crate::*;
use frame_support::assert_ok;
use sp_runtime::{FixedPointNumber, FixedU128};

mod add_liquidity;
mod add_token;
mod buy;
mod invariants;
mod remove_liquidity;
mod sell;

pub(crate) mod mock;
mod verification;

use mock::*;

#[macro_export]
macro_rules! assert_eq_approx {
	( $x:expr, $y:expr, $z:expr, $r:expr) => {{
		let diff = if $x >= $y { $x - $y } else { $y - $x };
		if diff > $z {
			panic!("\n{} not equal\n left: {:?}\nright: {:?}\n", $r, $x, $y);
		}
	}};
}

#[macro_export]
macro_rules! assert_balance {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(Tokens::free_balance($y, &$x), $z);
	}};
}

#[macro_export]
macro_rules! assert_balance_approx {
	( $x:expr, $y:expr, $z:expr, $l:expr) => {{
		let b = Tokens::free_balance($y, &$x);

		let diff = if $z >= b { $z - b } else { b - $z };
		if diff > $l {
			panic!("\nBalance not equal\n left: {}\nright: {}\n", b, $z);
		};
	}};
}

#[macro_export]
macro_rules! assert_pool_state {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(
			Tokens::free_balance(LRNA, &Omnipool::protocol_account()),
			$x,
			"Hub liquidity incorrect\n"
		);
		assert_eq!(TotalTVL::<Test>::get(), $y, "Total tvl incorrect\n");
		assert_eq!(HubAssetImbalance::<Test>::get(), $z, "Imbalance incorrect\n");
	}};
}

#[macro_export]
macro_rules! assert_pool_state_approx {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq_approx!(
			Tokens::free_balance(LRNA, &Omnipool::protocol_account()),
			$x,
			20u128,
			"Hub liquidity incorrect\n"
		);
		assert_eq_approx!(TotalTVL::<Test>::get(), $y, 20u128, "Total tvl incorrect\n");
		assert_eq!(HubAssetImbalance::<Test>::get(), $z, "Imbalance incorrect\n");
	}};
}

#[macro_export]
macro_rules! assert_asset_state {
	( $x:expr, $y:expr) => {{
		let reserve = Tokens::free_balance($x, &Omnipool::protocol_account());
		assert_eq!(reserve, $y.reserve);

		let actual = Assets::<Test>::get($x).unwrap();
		assert_eq!(actual, $y.into());
	}};
}
