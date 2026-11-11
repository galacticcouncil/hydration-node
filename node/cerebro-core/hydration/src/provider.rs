//! Plain substrate JSON-RPC client (`ureq`, behind the `rpc` feature), so cerebro analyzes any live
//! node without being inside one; without the feature it only replays recorded fixtures. Mirrors the pepl-shadow provider; adds an events read
//! and a storage-key scan used to enumerate pool accounts.

use serde_json::json;
use sp_core::H256;
use std::cell::RefCell;
use std::collections::BTreeMap;

/// `twox128("System") ++ twox128("Events")` — the System.Events storage key.
pub const EVENTS_KEY: &str = "0x26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7";

/// Optional record/replay layer over the JSON-RPC calls. Because EVERY chain read funnels through
/// `rpc()`, recording it captures a complete, deterministic fixture of a scan; replaying it drives
/// the whole pipeline offline (CI regression tests, no node). Keyed on `method|canonical-params`.
enum Cassette {
	#[cfg(feature = "rpc")]
	Live,
	#[cfg(feature = "rpc")]
	Record(RefCell<BTreeMap<String, serde_json::Value>>),
	Replay(BTreeMap<String, serde_json::Value>),
}

pub struct RemoteApi {
	#[cfg(feature = "rpc")]
	agent: ureq::Agent,
	#[cfg(feature = "rpc")]
	url: String,
	cassette: Cassette,
	/// Per-instance registry caches (see `balance::underlying_to_atoken`). Per-instance rather than
	/// process-static so replay fixtures stay isolated from each other in one test process.
	pub(crate) registry_cache: RefCell<crate::balance::RegistryCache>,
}

fn cassette_key(method: &str, params: &serde_json::Value) -> String {
	format!("{method}|{params}")
}

impl RemoteApi {
	#[cfg(feature = "rpc")]
	pub fn new(url: String) -> Self {
		let agent = ureq::AgentBuilder::new()
			.timeout(std::time::Duration::from_secs(180))
			.build();
		Self {
			agent,
			url,
			cassette: Cassette::Live,
			registry_cache: RefCell::new(Default::default()),
		}
	}

	/// Record every RPC call+response into a fixture file (written on `save_cassette`).
	#[cfg(feature = "rpc")]
	pub fn recording(url: String) -> Self {
		let mut me = Self::new(url);
		me.cassette = Cassette::Record(RefCell::new(BTreeMap::new()));
		me
	}

	/// Replay from a fixture file — no network. Any un-recorded call returns an error.
	pub fn replaying(fixture_json: &str) -> Result<Self, String> {
		let map: BTreeMap<String, serde_json::Value> =
			serde_json::from_str(fixture_json).map_err(|e| format!("cassette parse: {e}"))?;
		Ok(Self {
			#[cfg(feature = "rpc")]
			agent: ureq::Agent::new(),
			#[cfg(feature = "rpc")]
			url: String::new(),
			cassette: Cassette::Replay(map),
			registry_cache: RefCell::new(Default::default()),
		})
	}

	/// Serialize the recorded calls (for `recording` mode). Empty for other modes.
	#[cfg(feature = "rpc")]
	pub fn save_cassette(&self) -> String {
		match &self.cassette {
			Cassette::Record(m) => serde_json::to_string_pretty(&*m.borrow()).unwrap_or_default(),
			_ => String::new(),
		}
	}

	fn rpc(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
		match &self.cassette {
			Cassette::Replay(map) => map
				.get(&cassette_key(method, &params))
				.cloned()
				.ok_or_else(|| format!("cassette miss: {method} {params}")),
			#[cfg(feature = "rpc")]
			_ => self.live(method, params),
		}
	}

	#[cfg(feature = "rpc")]
	fn live(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
		let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
		let resp = self
			.agent
			.post(&self.url)
			.send_json(body)
			.map_err(|e| format!("{method}: transport: {e}"))?;
		let v: serde_json::Value = resp.into_json().map_err(|e| format!("{method}: bad json: {e}"))?;
		if let Some(err) = v.get("error") {
			return Err(format!("{method}: rpc error: {err}"));
		}
		let result = v.get("result").cloned().unwrap_or(serde_json::Value::Null);
		if let Cassette::Record(m) = &self.cassette {
			m.borrow_mut().insert(cassette_key(method, &params), result.clone());
		}
		Ok(result)
	}

	/// Best head as (hash, number).
	pub fn best_head(&self) -> Result<(H256, u32), String> {
		let h = self.rpc("chain_getBlockHash", json!([]))?;
		let h = h.as_str().ok_or("chain_getBlockHash: non-string result")?;
		let hash = parse_h256(h)?;
		let number = self.header_number(h)?;
		Ok((hash, number))
	}

	/// Last GRANDPA-finalized head as (hash, number). Scanning by number up to this tip is
	/// reorg-proof — a finalized block's canonical hash can never change.
	pub fn finalized_head(&self) -> Result<(H256, u32), String> {
		let h = self.rpc("chain_getFinalizedHead", json!([]))?;
		let h = h.as_str().ok_or("chain_getFinalizedHead: non-string result")?;
		let hash = parse_h256(h)?;
		let number = self.header_number(h)?;
		Ok((hash, number))
	}

	/// Canonical hash of block `number`, if the node knows it.
	pub fn block_hash(&self, number: u32) -> Result<Option<H256>, String> {
		let r = self.rpc("chain_getBlockHash", json!([number]))?;
		match r.as_str() {
			None => Ok(None),
			Some(s) => parse_h256(s).map(Some),
		}
	}

	/// Block number for a hash (used to interpolate a stableswap pool's amplification at `at`).
	pub fn header_number_of(&self, hash: H256) -> Result<u32, String> {
		self.header_number(&format!("{hash:?}"))
	}

	fn header_number(&self, hash_hex: &str) -> Result<u32, String> {
		let header = self.rpc("chain_getHeader", json!([hash_hex]))?;
		let num = header
			.get("number")
			.and_then(|n| n.as_str())
			.ok_or("chain_getHeader: missing number")?;
		u32::from_str_radix(num.trim_start_matches("0x"), 16).map_err(|e| format!("bad block number: {e}"))
	}

	pub fn system_chain(&self) -> Result<String, String> {
		let r = self.rpc("system_chain", json!([]))?;
		Ok(r.as_str().unwrap_or("?").to_string())
	}

	/// The runtime `spec_version` at `at` (via `state_getRuntimeVersion`). Used by the spec guard.
	pub fn runtime_spec_version(&self, at: H256) -> Result<u32, String> {
		let r = self.rpc("state_getRuntimeVersion", json!([format!("{at:?}")]))?;
		r.get("specVersion")
			.and_then(|v| v.as_u64())
			.map(|v| v as u32)
			.ok_or_else(|| "state_getRuntimeVersion: missing specVersion".to_string())
	}

	/// SCALE-encoded `System.Events` at `at`.
	pub fn events_raw(&self, at: H256) -> Result<Vec<u8>, String> {
		self.storage(EVENTS_KEY, at).map(|o| o.unwrap_or_default())
	}

	/// The block's extrinsics as raw SCALE bytes, in index order (matches `ApplyExtrinsic(i)`).
	pub fn block_extrinsics(&self, hash: H256) -> Result<Vec<Vec<u8>>, String> {
		let r = self.rpc("chain_getBlock", json!([format!("{hash:?}")]))?;
		let xts = r
			.get("block")
			.and_then(|b| b.get("extrinsics"))
			.and_then(|x| x.as_array())
			.ok_or("chain_getBlock: missing extrinsics")?;
		xts.iter()
			.map(|v| {
				let s = v.as_str().ok_or_else(|| "extrinsic: non-string".to_string())?;
				hex::decode(s.trim_start_matches("0x")).map_err(|e| format!("extrinsic: bad hex: {e}"))
			})
			.collect()
	}

	/// Raw storage value at a hex key.
	pub fn storage(&self, key_hex: &str, at: H256) -> Result<Option<Vec<u8>>, String> {
		let r = self.rpc("state_getStorage", json!([key_hex, format!("{at:?}")]))?;
		decode_opt_hex(r, "state_getStorage")
	}

	/// Invoke a runtime API (`state_call`) with SCALE-encoded args, returning the raw SCALE result.
	pub fn state_call(&self, method: &str, args: &[u8], at: H256) -> Result<Vec<u8>, String> {
		let r = self.rpc(
			"state_call",
			json!([method, format!("0x{}", hex::encode(args)), format!("{at:?}")]),
		)?;
		let s = r.as_str().ok_or_else(|| format!("{method}: non-string result"))?;
		hex::decode(s.trim_start_matches("0x")).map_err(|e| format!("{method}: bad hex: {e}"))
	}

	/// All storage keys under a hex prefix (paged). Used to enumerate pool-account maps.
	pub fn storage_keys(&self, prefix_hex: &str, at: H256) -> Result<Vec<Vec<u8>>, String> {
		let mut out = Vec::new();
		let mut start: Option<String> = None;
		loop {
			let params = match &start {
				Some(s) => json!([prefix_hex, 1000, s, format!("{at:?}")]),
				None => json!([prefix_hex, 1000, serde_json::Value::Null, format!("{at:?}")]),
			};
			let r = self.rpc("state_getKeysPaged", params)?;
			let arr = r.as_array().cloned().unwrap_or_default();
			if arr.is_empty() {
				break;
			}
			for k in &arr {
				if let Some(s) = k.as_str() {
					out.push(hex::decode(s.trim_start_matches("0x")).map_err(|e| format!("bad key hex: {e}"))?);
				}
			}
			start = arr.last().and_then(|v| v.as_str()).map(|s| s.to_string());
			if arr.len() < 1000 {
				break;
			}
		}
		Ok(out)
	}
}

impl crate::transport::Transport for RemoteApi {
	fn storage_many(&self, keys: &[Vec<u8>], at: cerebro_core::BlockHash) -> Result<Vec<Option<Vec<u8>>>, String> {
		// One `state_getStorage` per key keeps the recorded cassettes valid.
		keys.iter().map(|k| self.storage(&hex_key(k), H256(at))).collect()
	}

	fn evm_view(&self, to: [u8; 20], data: &[u8], at: cerebro_core::BlockHash) -> Result<Vec<u8>, String> {
		let raw = self.state_call(
			"EthereumRuntimeRPCApi_call",
			&crate::balance::encode_eth_call(to, data),
			H256(at),
		)?;
		crate::balance::decode_eth_call(&raw)
	}

	fn storage_keys(&self, prefix: &[u8], at: cerebro_core::BlockHash) -> Result<Vec<Vec<u8>>, String> {
		RemoteApi::storage_keys(self, &hex_key(prefix), H256(at))
	}

	fn block_number(&self, at: cerebro_core::BlockHash) -> Result<u32, String> {
		self.header_number_of(H256(at))
	}
}

fn hex_key(key: &[u8]) -> String {
	format!("0x{}", hex::encode(key))
}

fn decode_opt_hex(r: serde_json::Value, ctx: &str) -> Result<Option<Vec<u8>>, String> {
	match r.as_str() {
		None => Ok(None),
		Some(s) => hex::decode(s.trim_start_matches("0x"))
			.map(Some)
			.map_err(|e| format!("{ctx}: bad hex: {e}")),
	}
}

pub fn parse_h256(s: &str) -> Result<H256, String> {
	let b = hex::decode(s.trim_start_matches("0x")).map_err(|e| format!("bad hash hex: {e}"))?;
	if b.len() != 32 {
		return Err(format!("expected 32-byte hash, got {}", b.len()));
	}
	Ok(H256::from_slice(&b))
}
