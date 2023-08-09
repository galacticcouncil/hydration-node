use crate::types::Balance;
use hydra_dx_math::stableswap::types::AssetReserve;
use hydra_dx_math::to_u256;
use sp_core::{U256, U512};
use sp_runtime::traits::Zero;

mod add_liquidity;
mod amplification;
mod creation;
mod invariants;
pub(crate) mod mock;
mod remove_liquidity;
mod trades;
mod update_pool;

pub(crate) fn stable_swap_equation(d: Balance, amplification: Balance, reserves: &[AssetReserve]) -> bool {
	let n = reserves.len();
	let nn = n.pow(n as u32);
	let sum = reserves.iter().map(|v| v.amount).sum();
	let side1 = amplification
		.checked_mul(nn as u128)
		.unwrap()
		.checked_mul(sum)
		.unwrap()
		.checked_add(d)
		.unwrap();

	let amp = U512::from(amplification);
	let nn = U512::from(nn);
	let n = U512::from(n);
	let d = U512::from(d);

	let side2_01 = amp.checked_mul(nn).unwrap().checked_mul(d).unwrap();
	let nom = d.pow(n.checked_add(U512::one()).unwrap());

	let xp_hp: Vec<U512> = reserves
		.iter()
		.filter(|v| !(*v).is_zero())
		.map(|v| U512::from((*v).amount))
		.collect();
	let denom = xp_hp
		.iter()
		.try_fold(U512::one(), |acc, val| acc.checked_mul(*val))
		.unwrap();

	let r = nom.checked_div(denom).unwrap();

	let side2 = side2_01.checked_add(r).unwrap();

	//dbg!(side1);
	//dbg!(side2);
	true
}
