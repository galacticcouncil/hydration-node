#![cfg_attr(not(feature = "std"), no_std)]

//! Uniswap v3 simulator.
//!
//! The pool is sampled through the deployed QuoterV2 while the snapshot is taken
//! inside the runtime; the solver then interpolates the resulting curve natively.
//! No v3 swap math is reimplemented here.

use codec::Decode;
use codec::Encode;
use core::marker::PhantomData;
use evm::ExitReason;
use evm::ExitSucceed;
use frame_support::pallet_prelude::RuntimeDebug;
use hydra_dx_math::support::rational::{round_u512_to_rational, Rounding};
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{AmmSimulator, SimulatorError, TradeResult};
use hydradx_traits::evm::CallContext;
use hydradx_traits::router::{PoolEdge, PoolType};
use ice_support::AssetId;
use ice_support::Balance;
use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;
use precompile_utils::evm::writer::EvmDataWriter;
use primitive_types::{U256, U512};
use primitives::EvmAddress;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec;
use sp_std::vec::Vec;

const LOG_TARGET: &str = "uniswap_v3_simulator";

const VIEW_GAS_LIMIT: u64 = 250_000;
const QUOTE_GAS_LIMIT: u64 = 1_000_000;

/// `TickMath` price bounds. With no price limit of our own the quoter passes
/// `MIN_SQRT_RATIO + 1` / `MAX_SQRT_RATIO - 1`, so a quote that ends there
/// walked the pool dry and consumed less than it was asked to.
const MIN_SQRT_RATIO: u128 = 4_295_128_739;
const MAX_SQRT_RATIO: &str = "1461446703485210103287273052203988822378723970342";

/// Samples per direction. The ladder doubles, so it spans `2^(N-1)` of the
/// input token's virtual reserve down to dust.
///
/// ponytail: fixed ladder. Trades landing in the top intervals interpolate
/// across a 2x span and are under-quoted (never over — see `interpolate`);
/// add adaptive refinement guided by QuoterV2's `initializedTicksCrossed` if
/// measurements show the loss matters.
const SAMPLES_PER_DIRECTION: u32 = 16;

#[module_evm_utility_macro::generate_function_selector]
#[derive(Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	Token0 = "token0()",
	Token1 = "token1()",
	Fee = "fee()",
	Slot0 = "slot0()",
	Liquidity = "liquidity()",
	QuoteExactInputSingle = "quoteExactInputSingle((address,address,uint256,uint24,uint160))",
}

pub trait DataProvider {
	fn view(context: CallContext, data: Vec<u8>, gas: u64) -> (ExitReason, Vec<u8>);

	/// QuoterV2 address, or `None` when Uniswap is not configured.
	fn quoter() -> Option<EvmAddress>;

	/// Pools the solver may use. Read from the on-chain registry, not discovered.
	fn pools() -> Vec<EvmAddress>;

	fn address_to_asset(address: EvmAddress) -> Option<AssetId>;
}

/// A pool's sampled trade curve, taken at one block state.
///
/// Samples are `(cumulative_amount_in, cumulative_amount_out)` in ascending
/// order and are read only from the origin, never from a mid-curve offset.
#[derive(Clone, Encode, Decode, RuntimeDebug, PartialEq, Eq)]
pub struct PoolCurve {
	/// `token0` of the pool, which sorts first by EVM address.
	pub asset_a: AssetId,
	pub asset_b: AssetId,
	pub fee: u32,
	pub sqrt_price_x96: U256,
	pub a_to_b: Vec<(Balance, Balance)>,
	pub b_to_a: Vec<(Balance, Balance)>,
	/// Set once the solution has traded this pool, in either direction.
	///
	/// A second trade would have to be priced as `interpolate(total) -
	/// interpolate(consumed)`, and the difference of two chord under-estimates is
	/// not itself an under-estimate — it over-quotes by up to 30% in the top
	/// ladder interval, which then fails conservation at settlement.
	pub traded: bool,
}

#[derive(Clone, Encode, Decode, RuntimeDebug, PartialEq, Eq, Default)]
pub struct Snapshot {
	pub pools: BTreeMap<EvmAddress, PoolCurve>,
}

impl Snapshot {
	fn find(&self, asset_in: AssetId, asset_out: AssetId) -> Option<(&EvmAddress, &PoolCurve)> {
		self.pools.iter().find(|(_, curve)| {
			(curve.asset_a == asset_in && curve.asset_b == asset_out)
				|| (curve.asset_b == asset_in && curve.asset_a == asset_out)
		})
	}
}

/// Output for `amount` more input, on a curve that has already absorbed `consumed`.
///
/// Linear between the bracketing samples, but only where the curve is concave
/// there — concavity makes the chord a lower bound, and under-quoting is what
/// keeps conservation safe on execution. A convex interval (liquidity rising
/// past a tick) falls back to the interval's floor, which is a lower bound
/// unconditionally.
fn interpolate(samples: &[(Balance, Balance)], at: Balance) -> Option<Balance> {
	let last = samples.last()?;
	if at > last.0 {
		return None;
	}

	let mut lo = (0u128, 0u128);
	for (i, &(x_hi, y_hi)) in samples.iter().enumerate() {
		if at > x_hi {
			lo = (x_hi, y_hi);
			continue;
		}

		let (x_lo, y_lo) = lo;
		if at == x_hi {
			return Some(y_hi);
		}

		let span = x_hi.checked_sub(x_lo)?;
		if span == 0 {
			return Some(y_lo);
		}

		if !concave_at(samples, i, lo) {
			return Some(y_lo);
		}

		let step = U256::from(y_hi.checked_sub(y_lo)?)
			.checked_mul(U256::from(at.checked_sub(x_lo)?))?
			.checked_div(U256::from(span))?;

		return y_lo.checked_add(step.try_into().ok()?);
	}

	None
}

/// Whether the chord over interval `i` is no steeper than the one before it.
///
/// `lo` is interval `i`'s left endpoint, which is `(0, 0)` for the first one —
/// nothing precedes it, so it is taken as concave.
fn concave_at(samples: &[(Balance, Balance)], i: usize, lo: (Balance, Balance)) -> bool {
	let Some(prev) = i.checked_sub(1).and_then(|j| j.checked_sub(1).map(|k| samples[k])) else {
		return true;
	};
	let (x_hi, y_hi) = samples[i];

	let (Some(dx), Some(dy)) = (x_hi.checked_sub(lo.0), y_hi.checked_sub(lo.1)) else {
		return false;
	};
	let (Some(pdx), Some(pdy)) = (lo.0.checked_sub(prev.0), lo.1.checked_sub(prev.1)) else {
		return false;
	};
	if dx == 0 || pdx == 0 {
		return false;
	}

	// dy/dx <= pdy/pdx
	U512::from(dy) * U512::from(pdx) <= U512::from(pdy) * U512::from(dx)
}

/// Input needed for `amount_out`, by inverting the same curve.
///
/// The curve under-quotes output, so inverting it over-quotes input — conservative
/// in both directions.
fn invert(samples: &[(Balance, Balance)], target_out: Balance) -> Option<Balance> {
	let last = samples.last()?;
	if target_out > last.1 {
		return None;
	}

	let mut lo = (0u128, 0u128);
	for &(x_hi, y_hi) in samples.iter() {
		if target_out > y_hi {
			lo = (x_hi, y_hi);
			continue;
		}

		let (x_lo, y_lo) = lo;
		let span = y_hi.checked_sub(y_lo)?;
		if span == 0 {
			return Some(x_hi);
		}

		let step = U256::from(x_hi.checked_sub(x_lo)?)
			.checked_mul(U256::from(target_out.checked_sub(y_lo)?))?
			.checked_div(U256::from(span))?;

		// Round the input up: the caller must not be told it can buy for less.
		return x_lo.checked_add(step.try_into().ok()?).and_then(|x| x.checked_add(1));
	}

	None
}

/// One trade per pool per solution — see `PoolCurve::traded`.
fn ensure_untraded(curve: &PoolCurve) -> Result<(), SimulatorError> {
	if curve.traded {
		return Err(SimulatorError::NotSupported);
	}
	Ok(())
}

fn mark_traded(snapshot: &Snapshot, pool: &EvmAddress) -> Result<Snapshot, SimulatorError> {
	let mut updated = snapshot.clone();
	updated.pools.get_mut(pool).ok_or(SimulatorError::AssetNotFound)?.traded = true;
	Ok(updated)
}

/// A Uniswap v3 pool's immutable identity.
pub struct PoolMeta {
	pub token0: EvmAddress,
	pub token1: EvmAddress,
	pub asset_a: AssetId,
	pub asset_b: AssetId,
	pub fee: u32,
}

pub struct Simulator<DP>(PhantomData<DP>);

impl<DP: DataProvider> Simulator<DP> {
	fn call_word(pool: EvmAddress, function: Function, gas: u64) -> Option<U256> {
		let data = EvmDataWriter::new_with_selector(function).build();
		let (exit_reason, value) = DP::view(CallContext::new_view(pool), data, gas);
		if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) || value.len() < 32 {
			return None;
		}
		Some(U256::from_big_endian(&value[0..32]))
	}

	/// The pool's immutable identity: the tokens it trades, as both EVM addresses and
	/// asset ids, and its fee tier.
	///
	/// Three view calls, and the only part of a pool a caller that wants just the
	/// topology has to ask the chain for — the rest of `snapshot` is price and depth.
	/// Shared so the solver's snapshot and the oracle pricing graph cannot disagree
	/// about which assets a registered pool connects.
	pub fn pool_metadata(pool: EvmAddress) -> Option<PoolMeta> {
		let token0 = Self::address(pool, Function::Token0)?;
		let token1 = Self::address(pool, Function::Token1)?;
		Some(PoolMeta {
			asset_a: DP::address_to_asset(token0)?,
			asset_b: DP::address_to_asset(token1)?,
			fee: Self::call_word(pool, Function::Fee, VIEW_GAS_LIMIT)?.try_into().ok()?,
			token0,
			token1,
		})
	}

	fn address(pool: EvmAddress, function: Function) -> Option<EvmAddress> {
		Self::call_word(pool, function, VIEW_GAS_LIMIT).map(|w| {
			let bytes = w.to_big_endian();
			EvmAddress::from_slice(&bytes[12..32])
		})
	}

	/// `slot0().sqrtPriceX96` — the first word, low 160 bits.
	fn sqrt_price(pool: EvmAddress) -> Option<U256> {
		Self::call_word(pool, Function::Slot0, VIEW_GAS_LIMIT).map(|w| w & ((U256::one() << 160) - 1))
	}

	/// Virtual reserves at the current price: `x = L/√P`, `y = L·√P`.
	fn depth(liquidity: U256, sqrt_price_x96: U256) -> Option<(Balance, Balance)> {
		if sqrt_price_x96.is_zero() {
			return None;
		}
		let x = (liquidity << 96).checked_div(sqrt_price_x96)?;
		let y = liquidity.checked_mul(sqrt_price_x96)? >> 96;
		Some((x.try_into().ok()?, y.try_into().ok()?))
	}

	/// `(amount_out, truncated)` — `truncated` when the swap ran out of liquidity
	/// and so consumed less than `amount_in`. Such a sample must not be recorded:
	/// the solver would size a trade whose unconsumed remainder strands in the
	/// router account rather than reaching the user.
	fn quote(
		quoter: EvmAddress,
		token_in: EvmAddress,
		token_out: EvmAddress,
		fee: u32,
		amount_in: Balance,
	) -> Option<(Balance, bool)> {
		let data = EvmDataWriter::new_with_selector(Function::QuoteExactInputSingle)
			.write(token_in)
			.write(token_out)
			.write(U256::from(amount_in))
			.write(U256::from(fee))
			.write(U256::zero())
			.build();

		let (exit_reason, value) = DP::view(CallContext::new_view(quoter), data, QUOTE_GAS_LIMIT);
		if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) || value.len() < 32 {
			return None;
		}
		let amount_out: Balance = U256::from_big_endian(&value[0..32]).try_into().ok()?;

		// QuoterV2 answers with four words; a quoter that returns only the amount
		// leaves us unable to tell, so treat the sample as clean and rely on the
		// monotonicity break alone.
		let truncated = if value.len() >= 64 {
			let after = U256::from_big_endian(&value[32..64]);
			after <= U256::from(MIN_SQRT_RATIO).saturating_add(U256::one())
				|| after >= U256::from_dec_str(MAX_SQRT_RATIO).ok()?.saturating_sub(U256::one())
		} else {
			false
		};

		Some((amount_out, truncated))
	}

	/// Doubling ladder up to `max_in`, dropping samples the quoter cannot price.
	fn sample(
		quoter: EvmAddress,
		token_in: EvmAddress,
		token_out: EvmAddress,
		fee: u32,
		max_in: Balance,
	) -> Vec<(Balance, Balance)> {
		let mut samples = Vec::new();
		let smallest = max_in >> (SAMPLES_PER_DIRECTION - 1);
		if smallest == 0 {
			return samples;
		}

		let mut amount = smallest;
		for _ in 0..SAMPLES_PER_DIRECTION {
			let Some((out, truncated)) = Self::quote(quoter, token_in, token_out, fee, amount) else {
				break;
			};
			// A truncated quote priced less input than it was given, so the sample
			// does not describe this trade size. End the curve below it.
			if truncated {
				break;
			}
			// The curve must stay strictly increasing for interpolation and
			// inversion to bracket; a flat or zero step means the pool ran out.
			if out == 0 || samples.last().map(|&(_, y)| out <= y).unwrap_or(false) {
				break;
			}
			samples.push((amount, out));
			amount = amount.saturating_mul(2);
		}

		samples
	}
}

impl<DP: DataProvider> AmmSimulator for Simulator<DP> {
	type Snapshot = Snapshot;

	fn pool_type() -> PoolType<AssetId> {
		PoolType::UniswapV3(0)
	}

	fn matches_pool_type(pool_type: PoolType<AssetId>) -> bool {
		matches!(pool_type, PoolType::UniswapV3(_))
	}

	fn snapshot() -> Self::Snapshot {
		let mut snapshot = Snapshot::default();

		let Some(quoter) = DP::quoter() else {
			return snapshot;
		};

		for pool in DP::pools() {
			let Some(curve) = (|| {
				let PoolMeta {
					token0,
					token1,
					asset_a,
					asset_b,
					fee,
				} = Self::pool_metadata(pool)?;
				let sqrt_price_x96 = Self::sqrt_price(pool)?;
				let liquidity = Self::call_word(pool, Function::Liquidity, VIEW_GAS_LIMIT)?;
				let (max_a, max_b) = Self::depth(liquidity, sqrt_price_x96)?;

				Some(PoolCurve {
					asset_a,
					asset_b,
					fee,
					sqrt_price_x96,
					a_to_b: Self::sample(quoter, token0, token1, fee, max_a),
					b_to_a: Self::sample(quoter, token1, token0, fee, max_b),
					traded: false,
				})
			})() else {
				log::warn!(target: LOG_TARGET, "skipping unreadable pool {pool:?}");
				continue;
			};

			if curve.a_to_b.is_empty() || curve.b_to_a.is_empty() {
				log::warn!(target: LOG_TARGET, "skipping unquotable pool {pool:?}");
				continue;
			}

			// `find` keys on the pair alone, so a pair must map to one curve. Registering
			// a second fee tier would otherwise let a route tagged with one tier's fee be
			// priced off the other tier's curve and settle somewhere else.
			if snapshot
				.pools
				.values()
				.any(|c| c.asset_a == curve.asset_a && c.asset_b == curve.asset_b)
			{
				log::warn!(target: LOG_TARGET, "skipping pool {pool:?}: pair already registered");
				continue;
			}

			snapshot.pools.insert(pool, curve);
		}

		snapshot
	}

	fn simulate_sell(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_amount_out: Balance,
		snapshot: &Self::Snapshot,
	) -> Result<(Self::Snapshot, TradeResult), SimulatorError> {
		let (&pool, curve) = snapshot
			.find(asset_in, asset_out)
			.ok_or(SimulatorError::AssetNotFound)?;
		let forward = curve.asset_a == asset_in;

		ensure_untraded(curve)?;
		let samples = if forward { &curve.a_to_b } else { &curve.b_to_a };

		let amount_out = interpolate(samples, amount_in).ok_or(SimulatorError::TradeTooLarge)?;

		if amount_out < min_amount_out {
			return Err(SimulatorError::LimitNotMet);
		}

		Ok((mark_traded(snapshot, &pool)?, TradeResult::new(amount_in, amount_out)))
	}

	fn simulate_buy(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_amount_in: Balance,
		snapshot: &Self::Snapshot,
	) -> Result<(Self::Snapshot, TradeResult), SimulatorError> {
		let (&pool, curve) = snapshot
			.find(asset_in, asset_out)
			.ok_or(SimulatorError::AssetNotFound)?;
		let forward = curve.asset_a == asset_in;

		ensure_untraded(curve)?;
		let samples = if forward { &curve.a_to_b } else { &curve.b_to_a };

		let amount_in = invert(samples, amount_out).ok_or(SimulatorError::TradeTooLarge)?;

		if amount_in > max_amount_in {
			return Err(SimulatorError::LimitNotMet);
		}

		Ok((mark_traded(snapshot, &pool)?, TradeResult::new(amount_in, amount_out)))
	}

	/// Marginal price from `slot0`, exact and fee-free — matching the omnipool
	/// simulator, whose price the netting pass uses to value matched volume.
	fn get_spot_price(
		asset_in: AssetId,
		asset_out: AssetId,
		snapshot: &Self::Snapshot,
	) -> Result<Ratio, SimulatorError> {
		let (_, curve) = snapshot
			.find(asset_in, asset_out)
			.ok_or(SimulatorError::AssetNotFound)?;

		// slot0 gives token1 per token0, as (sqrtPriceX96 / 2^96)^2.
		let sqrt = U512::from(curve.sqrt_price_x96);
		let squared = sqrt.checked_mul(sqrt).ok_or(SimulatorError::MathError)?;
		let q192 = U512::one() << 192;

		let (n, d) = if curve.asset_a == asset_in {
			(squared, q192)
		} else {
			(q192, squared)
		};

		let (n, d) = round_u512_to_rational((n, d), Rounding::Nearest);
		Ok(Ratio::new(n, d))
	}

	fn can_trade(asset_in: AssetId, asset_out: AssetId, snapshot: &Self::Snapshot) -> Option<PoolType<AssetId>> {
		snapshot
			.find(asset_in, asset_out)
			.map(|(_, curve)| PoolType::UniswapV3(curve.fee))
	}

	fn pool_edges(snapshot: &Self::Snapshot) -> Vec<PoolEdge<AssetId>> {
		snapshot
			.pools
			.values()
			.map(|curve| PoolEdge {
				pool_type: PoolType::UniswapV3(curve.fee),
				assets: vec![curve.asset_a, curve.asset_b],
			})
			.collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Canned pool: price 1, so both virtual reserves equal `LIQUIDITY`, and the
	/// quoter pays 2 out per 1 in until the pool is drained.
	const POOL: EvmAddress = EvmAddress::repeat_byte(1);
	/// Same token pair as `POOL`, standing in for a second registered fee tier.
	const POOL_SAME_PAIR: EvmAddress = EvmAddress::repeat_byte(3);
	const QUOTER: EvmAddress = EvmAddress::repeat_byte(2);
	const TOKEN0: EvmAddress = EvmAddress::repeat_byte(0x10);
	const TOKEN1: EvmAddress = EvmAddress::repeat_byte(0x11);
	const FEE: u32 = 3000;
	/// `smallest = LIQUIDITY >> 15 = 1_000_000`, so the ladder is
	/// 1e6, 2e6, … 32_768e6 — exactly `SAMPLES_PER_DIRECTION` steps.
	const LIQUIDITY: Balance = 32_768_000_000;

	/// How much input this pool can absorb before the swap walks it dry.
	trait Scenario {
		const CAP: Balance;
	}

	struct Deep;
	impl Scenario for Deep {
		const CAP: Balance = Balance::MAX;
	}

	/// Drains between the 5th ladder step (16e6) and the 6th (32e6).
	struct Shallow;
	impl Scenario for Shallow {
		const CAP: Balance = 20_000_000;
	}

	struct Mock<S>(PhantomData<S>);

	impl<S: Scenario> DataProvider for Mock<S> {
		fn view(_context: CallContext, data: Vec<u8>, _gas: u64) -> (ExitReason, Vec<u8>) {
			let ok = ExitReason::Succeed(ExitSucceed::Returned);
			let selector = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
			let word = |v: U256| v.to_big_endian().to_vec();

			match Function::try_from(selector).expect("unexpected selector") {
				Function::Token0 => (ok, word(U256::from_big_endian(TOKEN0.as_bytes()))),
				Function::Token1 => (ok, word(U256::from_big_endian(TOKEN1.as_bytes()))),
				Function::Fee => (ok, word(U256::from(FEE))),
				Function::Slot0 => (ok, word(U256::one() << 96)),
				Function::Liquidity => (ok, word(U256::from(LIQUIDITY))),
				Function::QuoteExactInputSingle => {
					let amount_in = U256::from_big_endian(&data[68..100]).as_u128();
					let consumed = amount_in.min(S::CAP);
					let truncated = amount_in > S::CAP;

					let mut out = word(U256::from(consumed * 2));
					// sqrtPriceX96After: the min bound once the pool is drained.
					out.extend(word(if truncated {
						U256::from(MIN_SQRT_RATIO) + U256::one()
					} else {
						U256::one() << 96
					}));
					out.extend(word(U256::zero())); // initializedTicksCrossed
					out.extend(word(U256::from(100_000u64))); // gasEstimate
					(ok, out)
				}
			}
		}

		fn quoter() -> Option<EvmAddress> {
			Some(QUOTER)
		}

		fn pools() -> Vec<EvmAddress> {
			vec![POOL, POOL_SAME_PAIR]
		}

		fn address_to_asset(address: EvmAddress) -> Option<AssetId> {
			match address {
				a if a == TOKEN0 => Some(1),
				a if a == TOKEN1 => Some(2),
				_ => None,
			}
		}
	}

	#[test]
	fn snapshot_should_sample_the_whole_ladder_when_no_quote_is_truncated() {
		let snapshot = Simulator::<Mock<Deep>>::snapshot();
		let curve = snapshot.pools.get(&POOL).expect("pool to be sampled");

		assert_eq!(curve.a_to_b.len(), SAMPLES_PER_DIRECTION as usize);
		assert_eq!(curve.a_to_b[0], (1_000_000, 2_000_000));
		assert_eq!(curve.a_to_b[15], (32_768_000_000, 65_536_000_000));
	}

	/// The truncated sample must not be recorded: it priced less input than it was
	/// given, so a solver sizing a trade there would strand the remainder.
	#[test]
	fn snapshot_should_end_the_curve_below_the_first_truncated_quote() {
		let snapshot = Simulator::<Mock<Shallow>>::snapshot();
		let curve = snapshot.pools.get(&POOL).expect("pool to be sampled");

		assert_eq!(curve.a_to_b.len(), 5);
		assert_eq!(curve.a_to_b[4], (16_000_000, 32_000_000));
	}

	/// A node built before this simulator existed decodes the shipped snapshot as
	/// the shorter tuple it knows. SCALE concatenates tuple fields with no length
	/// prefix and `Decode::decode` stops once its own type is satisfied, so the
	/// older node reads the simulators it knows and ignores the rest.
	///
	/// This only holds while new simulators are **appended** to
	/// `HydrationSimulators`. Inserting one in the middle shifts every field after
	/// it and silently corrupts an older node's decode.
	#[test]
	fn appending_a_simulator_should_leave_the_snapshot_decodable_by_an_older_node() {
		let pool = PoolCurve {
			asset_a: 1,
			asset_b: 2,
			fee: 3000,
			sqrt_price_x96: U256::one() << 96,
			a_to_b: vec![(1, 2)],
			b_to_a: vec![(2, 1)],
			traded: false,
		};
		let mut added = Snapshot::default();
		added.pools.insert(EvmAddress::repeat_byte(7), pool);

		// Stand-ins for the three simulators that already shipped.
		let new_state = (1u32, 2u64, 3u128, added);
		let encoded = new_state.encode();

		let old_state: (u32, u64, u128) =
			Decode::decode(&mut &encoded[..]).expect("an older node must still decode the prefix");
		assert_eq!(old_state, (1u32, 2u64, 3u128));
	}

	/// Past the curve the simulator must refuse rather than extrapolate.
	#[test]
	fn simulate_sell_should_fail_when_amount_is_beyond_the_truncated_curve() {
		let snapshot = Simulator::<Mock<Shallow>>::snapshot();

		assert_eq!(
			Simulator::<Mock<Shallow>>::simulate_sell(1, 2, 20_000_000, 0, &snapshot).map(|(_, r)| r),
			Err(SimulatorError::TradeTooLarge)
		);
	}

	/// Concave: each doubling of input buys less than the one before.
	fn concave() -> Vec<(Balance, Balance)> {
		vec![(100, 100), (200, 190), (400, 360), (800, 680)]
	}

	#[test]
	fn interpolate_should_return_sample_when_amount_is_on_a_sample() {
		assert_eq!(interpolate(&concave(), 400), Some(360));
	}

	#[test]
	fn interpolate_should_be_linear_between_samples_when_curve_is_concave() {
		// halfway across [400, 800] -> 360 + (680-360)/2
		assert_eq!(interpolate(&concave(), 600), Some(520));
	}

	#[test]
	fn interpolate_should_not_exceed_the_true_curve_when_curve_is_concave() {
		let samples = concave();
		for at in [150u128, 250, 333, 500, 600, 799] {
			let interpolated = interpolate(&samples, at).unwrap();
			let (_, y_lo) = samples.iter().rev().find(|&&(x, _)| x <= at).copied().unwrap_or((0, 0));
			// never above the chord's right endpoint, never below its left
			assert!(interpolated >= y_lo, "at {at}: {interpolated} < {y_lo}");
			let (_, y_hi) = samples.iter().find(|&&(x, _)| x >= at).copied().unwrap();
			assert!(interpolated <= y_hi, "at {at}: {interpolated} > {y_hi}");
		}
	}

	#[test]
	fn interpolate_should_fall_back_to_the_floor_when_interval_is_convex() {
		// third interval buys more per unit than the second - liquidity rose.
		let samples = vec![(100, 100), (200, 190), (400, 400), (800, 700)];
		assert_eq!(interpolate(&samples, 300), Some(190));
	}

	#[test]
	fn interpolate_should_fail_when_amount_exceeds_last_sample() {
		assert_eq!(interpolate(&concave(), 801), None);
	}

	#[test]
	fn interpolate_should_be_monotone_when_input_grows() {
		let samples = concave();
		let mut previous = 0;
		for at in 1..=800u128 {
			let out = interpolate(&samples, at).unwrap();
			assert!(out >= previous, "at {at}: {out} < {previous}");
			previous = out;
		}
	}

	#[test]
	fn invert_should_overstate_input_when_target_is_between_samples() {
		// 520 out sits halfway across [400, 800]; the exact inverse is 600.
		assert_eq!(invert(&concave(), 520), Some(601));
	}

	#[test]
	fn invert_should_fail_when_target_exceeds_last_sample() {
		assert_eq!(invert(&concave(), 681), None);
	}

	/// Why a pool is traded once per solution.
	///
	/// Both interpolated points sit below the true curve, and the one at `consumed`
	/// is dragged down further, so their difference lands *above* the truth — the
	/// opposite of the under-quoting the conservation check relies on. Reading the
	/// same curve from the origin stays a lower bound, as the last two asserts pin.
	#[test]
	fn differencing_should_overquote_the_true_increment_when_legs_are_split() {
		// Constant product at price 1, on the ladder the simulator samples.
		const L: Balance = 32_768_000_000;
		let out = |x: Balance| L * x / (L + x);
		let samples: Vec<(Balance, Balance)> = (0..SAMPLES_PER_DIRECTION)
			.map(|i| {
				let x = (L >> (SAMPLES_PER_DIRECTION - 1)) << i;
				(x, out(x))
			})
			.collect();

		let consumed = 31_948_800_000;
		let amount = 327_680_000;
		let differenced = interpolate(&samples, consumed + amount).unwrap() - interpolate(&samples, consumed).unwrap();

		assert_eq!(differenced, 109_226_666);
		assert_eq!(out(consumed + amount) - out(consumed), 83_583_841);

		assert_eq!(interpolate(&samples, amount).unwrap(), 324_045_623);
		assert_eq!(out(amount), 324_435_643);
	}

	#[test]
	fn snapshot_should_keep_one_curve_when_a_pair_is_registered_twice() {
		let snapshot = Simulator::<Mock<Deep>>::snapshot();

		assert_eq!(snapshot.pools.len(), 1);
		assert!(snapshot.pools.contains_key(&POOL));
		assert!(!snapshot.pools.contains_key(&POOL_SAME_PAIR));
	}

	#[test]
	fn simulate_sell_should_fail_when_the_pool_was_already_traded() {
		let snapshot = Simulator::<Mock<Deep>>::snapshot();
		let (traded, _) =
			Simulator::<Mock<Deep>>::simulate_sell(1, 2, 1_000_000, 0, &snapshot).expect("first sell to price");

		assert_eq!(
			Simulator::<Mock<Deep>>::simulate_sell(1, 2, 1_000_000, 0, &traded),
			Err(SimulatorError::NotSupported)
		);
	}

	/// The opposite direction is refused for the same reason: the curve is read from
	/// the origin and knows nothing about a pool the batch has already pushed.
	#[test]
	fn simulate_buy_should_fail_when_the_pool_was_traded_in_the_other_direction() {
		let snapshot = Simulator::<Mock<Deep>>::snapshot();
		let (traded, _) =
			Simulator::<Mock<Deep>>::simulate_sell(1, 2, 1_000_000, 0, &snapshot).expect("first sell to price");

		assert_eq!(
			Simulator::<Mock<Deep>>::simulate_buy(2, 1, 1_000_000, Balance::MAX, &traded),
			Err(SimulatorError::NotSupported)
		);
	}
}
