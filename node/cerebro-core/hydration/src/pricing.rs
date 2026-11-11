//! Asset valuation via Hydration's own on-chain oracle — no reimplementation.
//!
//! The runtime exposes oracle prices through the Chainlink adapter precompile. We compute the
//! oracle's EVM address locally (a fixed 20-byte layout) and read its `latestAnswer()` through the
//! exposed `EthereumRuntimeRPCApi::call` runtime API via `state_call`. That runs the runtime's real
//! price logic (route EMA price + registry-decimals adjustment), so our numbers can't drift from
//! what the chain itself uses. Price is `USDT per 1 whole asset`, scaled to 8 decimals.

use crate::provider::RemoteApi;
use crate::transport::Transport;
use cerebro_core::reader::PriceSource;
use cerebro_core::value::Usd6;
use cerebro_core::BlockHash;
use codec::{Decode, Encode};
use hydradx_runtime::Runtime;
use hydradx_traits::OraclePeriod;
use pallet_asset_registry::Assets as RegistryAssets;
use primitive_types::U512;
use primitives::AssetId;
use sp_core::{H160, H256, U256};
use std::collections::HashMap;

pub const USDT: AssetId = 10;
/// Validated non-stable majors trusted as divergence anchors (HDX native, DOT vs its own trades).
/// Stablecoins are trusted dynamically via `is_trusted` (oracle price ≈ $1), so no id list to chase.
const TRUSTED_MAJORS: [AssetId; 2] = [0, 5]; // HDX, DOT
const EMPTY_SOURCE: [u8; 8] = [0u8; 8];
const PERIOD: OraclePeriod = OraclePeriod::Short;
/// `latestAnswer()` selector (keccak256("latestAnswer()")[..4]).
const LATEST_ANSWER: [u8; 4] = [0x50, 0xd2, 0x5b, 0xcd];
const ANSWER_SCALE: u128 = 100_000_000; // oracle answers carry 8 decimals

type AssetDetailsOf = pallet_asset_registry::AssetDetails<<Runtime as pallet_asset_registry::Config>::StringLimit>;

/// Oracle feed address for `(a, b, period, source)`: marker + period + source + the two asset ids.
fn oracle_address(a: AssetId, b: AssetId, period: OraclePeriod, source: [u8; 8]) -> H160 {
	let mut o = [0u8; 20];
	o[2] = 1;
	o[3] = period.encode()[0];
	o[4..12].copy_from_slice(&source);
	o[12..16].copy_from_slice(&a.to_be_bytes());
	o[16..20].copy_from_slice(&b.to_be_bytes());
	H160::from(o)
}

/// Per-block price snapshot: `USDT per 1 whole asset` (×1e8), the asset's decimals, and its symbol.
pub struct Prices {
	answer_1e8: HashMap<AssetId, u128>,
	decimals: HashMap<AssetId, u32>,
	symbols: HashMap<AssetId, String>,
}

impl Prices {
	pub fn fetch<T: Transport>(api: &T, at: H256, assets: impl IntoIterator<Item = AssetId>) -> Self {
		let mut answer_1e8 = HashMap::new();
		let mut decimals = HashMap::new();
		let mut symbols = HashMap::new();
		let mut want: Vec<AssetId> = assets.into_iter().collect();
		want.push(USDT);
		want.sort_unstable();
		want.dedup();
		for a in want {
			if let Some((dec, sym)) = fetch_asset_meta(api, at, a) {
				decimals.insert(a, dec);
				if let Some(s) = sym {
					symbols.insert(a, s);
				}
			}
			if a == USDT {
				answer_1e8.insert(a, ANSWER_SCALE); // USDT in USDT = 1.0
			} else if let Some(ans) = fetch_answer(api, at, a) {
				answer_1e8.insert(a, ans);
			}
		}
		Self {
			answer_1e8,
			decimals,
			symbols,
		}
	}

	/// Whether `asset` was asked for (priced or not) in this snapshot.
	pub fn has_decimals(&self, asset: AssetId) -> bool {
		self.decimals.contains_key(&asset)
	}

	/// Fold `other` into this snapshot; `other` wins on overlap.
	pub fn extend(&mut self, other: Prices) {
		self.answer_1e8.extend(other.answer_1e8);
		self.decimals.extend(other.decimals);
		self.symbols.extend(other.symbols);
	}

	pub fn empty() -> Self {
		Self {
			answer_1e8: HashMap::new(),
			decimals: HashMap::new(),
			symbols: HashMap::new(),
		}
	}

	/// Readable name for an asset: its registry symbol, or `#id` if unknown.
	pub fn symbol(&self, asset: AssetId) -> String {
		self.symbols.get(&asset).cloned().unwrap_or_else(|| format!("#{asset}"))
	}

	/// Value `amount` (raw units, signed) of `asset` in USDT raw units (6 decimals). None if unpriced.
	pub fn value_usdt(&self, asset: AssetId, amount: i128) -> Option<i128> {
		let answer = *self.answer_1e8.get(&asset)?;
		let dec = *self.decimals.get(&asset)?;
		// raw_usdt = amount * answer / 10^(dec+2)  [= *1e6 / (1e8 * 10^dec)]
		let denom = U512::from(10u8).pow(U512::from(dec + 2));
		if denom.is_zero() {
			return None;
		}
		let mag = U512::from(amount.unsigned_abs()) * U512::from(answer) / denom;
		if mag.bits() > 127 {
			return None;
		}
		let v = mag.low_u128() as i128;
		Some(if amount < 0 { -v } else { v })
	}

	pub fn can_price(&self, asset: AssetId) -> bool {
		self.answer_1e8.contains_key(&asset) && self.decimals.contains_key(&asset)
	}

	pub fn answer(&self, asset: AssetId) -> Option<u128> {
		self.answer_1e8.get(&asset).copied()
	}

	/// USD price of one whole unit of `asset` (the oracle answer, unscaled).
	pub fn price_usd(&self, asset: AssetId) -> Option<f64> {
		self.answer_1e8.get(&asset).map(|a| *a as f64 / ANSWER_SCALE as f64)
	}

	/// A trusted divergence anchor: a validated major (DOT/HDX), or a stablecoin whose oracle price
	/// reads ≈ $1 (catches USDT/USDC and any correctly-pegged stable; excludes a mispriced one).
	pub fn is_trusted(&self, asset: AssetId) -> bool {
		TRUSTED_MAJORS.contains(&asset) || matches!(self.price_usd(asset), Some(p) if (0.98..=1.02).contains(&p))
	}

	/// Strictly a ≈ $1 stablecoin by oracle price — no majors. Used to anchor stable pools at par
	/// (DOT/HDX must NOT qualify, or a DOT pool would be mis-anchored at $1).
	pub fn is_dollar_peg(&self, asset: AssetId) -> bool {
		matches!(self.price_usd(asset), Some(p) if (0.98..=1.02).contains(&p))
	}

	pub fn decimals_of(&self, asset: AssetId) -> Option<u32> {
		self.decimals.get(&asset).copied()
	}
}

/// Lightweight asset-symbol map (registry read only, no oracle) for readable log output.
pub struct Symbols(HashMap<AssetId, String>);

impl Symbols {
	pub fn fetch<T: Transport>(api: &T, at: H256, assets: impl IntoIterator<Item = AssetId>) -> Self {
		let mut m = HashMap::new();
		for a in assets {
			if let Some((_, Some(sym))) = fetch_asset_meta(api, at, a) {
				m.insert(a, sym);
			}
		}
		Self(m)
	}

	pub fn symbol(&self, asset: AssetId) -> String {
		self.0.get(&asset).cloned().unwrap_or_else(|| format!("#{asset}"))
	}
}

/// Registry decimals for one asset (no oracle). Used to normalize pool reserves to a common scale.
pub fn decimals<T: Transport>(api: &T, at: H256, asset: AssetId) -> Option<u32> {
	fetch_asset_meta(api, at, asset).map(|(dec, _)| dec)
}

fn fetch_asset_meta<T: Transport>(api: &T, at: H256, asset: AssetId) -> Option<(u32, Option<String>)> {
	let key = RegistryAssets::<Runtime>::hashed_key_for(asset);
	let raw = api.storage(&key, at.0).ok()??;
	let details: AssetDetailsOf = Decode::decode(&mut &raw[..]).ok()?;
	let dec = details.decimals.map(|d| d as u32)?;
	let sym = details.symbol.map(|s| String::from_utf8_lossy(&s).into_owned());
	Some((dec, sym))
}

/// Read `latestAnswer()` on the (USDT, asset) oracle feed via the EVM-call runtime API.
/// The feed `(a, b)` returns "whole a per whole b", so `(USDT, asset)` yields USDT per whole asset.
fn fetch_answer<T: Transport>(api: &T, at: H256, asset: AssetId) -> Option<u128> {
	fetch_answer_src(api, at, asset, EMPTY_SOURCE)
}

/// Same, but with an explicit oracle source (EMPTY = router-route price, OMNIPOOL = direct omnipool).
fn fetch_answer_src<T: Transport>(api: &T, at: H256, asset: AssetId, source: [u8; 8]) -> Option<u128> {
	let to = oracle_address(USDT, asset, PERIOD, source);
	let value = api.evm_view(to.0, &LATEST_ANSWER, at.0).ok()?;
	if value.len() < 32 {
		return None;
	}
	Some(U256::from_big_endian(&value[..32]).low_u128())
}

/// Calibration: print USD prices for a known set to verify against reality (USDT = $1.00,
/// and each price should match the rate of any real trade in that asset).
pub fn dump_prices(api: &RemoteApi, at: H256) {
	let known = [
		(0u32, "HDX"),
		(5, "DOT"),
		(10, "USDT"),
		(22, "USDC"),
		(222, "HOLLAR"),
		(18, "DAI-mrl"),
	];
	let omnipool_source = primitives::constants::chain::OMNIPOOL_SOURCE;
	log::info!(target: "cerebro", "--- oracle price by SOURCE (Short period, USD per whole unit) ---");
	log::info!(target: "cerebro", "  {:<20} {:>16} {:>16}", "asset", "route(EMPTY)", "omnipool");
	for (asset, sym) in known {
		let route = fetch_answer_src(api, at, asset, EMPTY_SOURCE).map(|a| a as f64 / ANSWER_SCALE as f64);
		let omni = fetch_answer_src(api, at, asset, omnipool_source).map(|a| a as f64 / ANSWER_SCALE as f64);
		let fmt = |v: Option<f64>| v.map(|x| format!("${x:.4}")).unwrap_or_else(|| "—".into());
		log::info!(
			target: "cerebro",
			"  {:<20} {:>16} {:>16}",
			format!("{sym} ({asset})"),
			fmt(route),
			fmt(omni),
		);
	}
}

/// `PriceSource` over a pre-fetched per-block oracle snapshot. The `at` arg is redundant here (the
/// snapshot is already pinned to one block) and ignored; the future simulator-spot backend can
/// implement the same trait over the AMM snapshot with core untouched.
impl PriceSource for Prices {
	fn value_usd(&self, asset: cerebro_core::AssetId, amount: i128, _at: BlockHash) -> Option<Usd6> {
		self.value_usdt(asset, amount).map(Usd6)
	}
}
