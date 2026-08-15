use ethabi::ethereum_types::U512;
use sp_core::U256;

pub const OCTILL: u128 = 10u128.pow(27);
pub const QUINTILL: u128 = 10u128.pow(18);

/// Largest `e` such that `10^e` fits in `U256` / `U512`.
const MAX_POW10_U256: u8 = 77;
const MAX_POW10_U512: u8 = 154;

/// `10^exp`, `None` when the result would not fit.
///
/// `uint`'s `pow` panics on overflow in release builds too, so exponents derived from a reserve's
/// `decimals` — an unvalidated byte of the on-chain configuration word — must never reach it
/// directly.
pub fn pow10_u256(exp: u8) -> Option<U256> {
	(exp <= MAX_POW10_U256).then(|| U256::from(10).pow(exp.into()))
}

/// `10^exp`, `None` when the result would not fit. See [`pow10_u256`].
pub fn pow10_u512(exp: u8) -> Option<U512> {
	(exp <= MAX_POW10_U512).then(|| U512::from(10).pow(exp.into()))
}

/// Multiplies two ray, rounding half up to the nearest ray.
pub fn ray_mul(a: U256, b: U256) -> Option<U256> {
	if a.is_zero() || b.is_zero() {
		return Some(U256::zero());
	}

	let ray = U512::from(OCTILL);
	let res512 = a.full_mul(b).checked_add(ray / 2)?.checked_div(ray)?;

	res512.try_into().ok()
}

/// Executes a percentage multiplication.
/// Params:
///     value: The value of which the percentage needs to be calculated
///     percentage: The percentage of the value to be calculated, in basis points.
pub fn percent_mul(value: U256, percentage: U256) -> Option<U256> {
	if percentage.is_zero() {
		return Some(U256::zero());
	}

	let percentage_factor = U512::from(10u128.pow(4));
	let half_percentage_factor = percentage_factor / 2;
	let nominator = value.full_mul(percentage).checked_add(half_percentage_factor);
	let res: U512 = nominator.and_then(|n| n.checked_div(percentage_factor))?;

	res.try_into().ok()
}

/// Divides two wad, rounding half up to the nearest wad.
pub fn wad_div(a: U256, b: U256) -> Option<U256> {
	if a.is_zero() {
		return Some(U256::zero());
	}
	if b.is_zero() {
		return None;
	}

	let wad = U256::from(QUINTILL);
	let nominator = a.full_mul(wad).checked_add(U512::from(b / 2));
	let res = nominator.and_then(|n| n.checked_div(U512::from(b)))?;

	res.try_into().ok()
}
