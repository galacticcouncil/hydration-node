mod math;

#[cfg(test)]
pub mod tests;

pub mod types;

use crate::stableswap::types::AssetReserve;
use crate::types::Balance;
pub use math::*;
use primitive_types::U512;

pub fn stable_swap_equation(d: Balance, amplification: Balance, reserves: &[AssetReserve]) -> bool {
	let balances = normalize_reserves(reserves);
	let n = reserves.len();
	let nn = n.pow(n as u32);
	let sum = balances.iter().sum();
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

	let xp_hp: Vec<U512> = balances.iter().filter(|v| !*v != 0).map(|v| U512::from(*v)).collect();
	let denom = xp_hp
		.iter()
		.try_fold(U512::one(), |acc, val| acc.checked_mul(*val))
		.unwrap();

	let denom = nn.checked_mul(denom).unwrap();
	let r = nom.checked_div(denom).unwrap();
	let side2 = side2_01.checked_add(r).unwrap();
	let diff = U512::from(side1).abs_diff(side2);
	//dbg!(side1, side2, diff);
	diff <= U512::from(100_000)
}
