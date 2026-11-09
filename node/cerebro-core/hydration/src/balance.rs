//! Ground-truth balance reads from state (`state_getStorage` / EVM `balanceOf`) — the authority the
//! event-derived deltas are confirmed against. Also resolves aToken contracts and Aave reserves.

use crate::provider::RemoteApi;
use crate::transport::Transport;
use cerebro_core::{AssetId, Balance, BlockHash, NATIVE_ASSET};
use codec::{Decode, Encode};
use fp_evm::{CallInfo, ExitReason};
use sp_core::hashing::{blake2_128, twox_128, twox_64};
use sp_core::{H160, H256, U256};

pub(crate) fn h256(b: BlockHash) -> H256 {
	H256(b)
}

/// `balanceOf(address)` selector.
const BALANCE_OF: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];
/// `UNDERLYING_ASSET_ADDRESS()` selector — the Aave aToken standard getter.
const UNDERLYING_ASSET_ADDRESS: [u8; 4] = [0xb1, 0x6a, 0x19, 0xde];

/// ERC20 `balanceOf(who)` via the `EthereumRuntimeRPCApi::call` runtime API — the authoritative
/// ledger for aTokens and any EVM-native token whose movements emit no Substrate event.
pub fn erc20_balance_of<T: Transport>(api: &T, contract: H160, who: H160, at: H256) -> Option<u128> {
	let mut data = BALANCE_OF.to_vec();
	data.extend_from_slice(&[0u8; 12]);
	data.extend_from_slice(who.as_bytes());
	let out = eth_call_bytes(api, contract, data, at)?;
	if out.len() < 32 {
		return None;
	}
	Some(U256::from_big_endian(&out[..32]).low_u128())
}

/// Raw `eth_call`-style read via the runtime API. `None` on revert or transport failure.
fn eth_call_bytes<T: Transport>(api: &T, contract: H160, data: Vec<u8>, at: H256) -> Option<Vec<u8>> {
	api.evm_view(contract.0, &data, at.0).ok()
}

/// SCALE-encode the 11 args of `EthereumRuntimeRPCApi::call` for a read from the zero address
/// (estimate=true).
pub fn encode_eth_call(to: [u8; 20], data: &[u8]) -> Vec<u8> {
	let mut a = Vec::new();
	H160::zero().encode_to(&mut a);
	H160(to).encode_to(&mut a);
	data.to_vec().encode_to(&mut a);
	U256::zero().encode_to(&mut a);
	U256::from(10_000_000u64).encode_to(&mut a);
	Option::<U256>::None.encode_to(&mut a);
	Option::<U256>::None.encode_to(&mut a);
	Option::<U256>::None.encode_to(&mut a);
	true.encode_to(&mut a);
	Option::<Vec<(H160, Vec<H256>)>>::None.encode_to(&mut a);
	a.push(0u8);
	a
}

/// The output of a successful `EthereumRuntimeRPCApi::call`, from its SCALE-encoded result.
pub fn decode_eth_call(raw: &[u8]) -> Result<Vec<u8>, String> {
	let res: Result<CallInfo, sp_runtime::DispatchError> =
		Decode::decode(&mut &raw[..]).map_err(|e| format!("EthereumRuntimeRPCApi_call: {e}"))?;
	call_output(res.map_err(|e| format!("EthereumRuntimeRPCApi_call: {e:?}"))?)
}

/// The output bytes of an EVM call, or why there are none.
pub fn call_output(info: CallInfo) -> Result<Vec<u8>, String> {
	match info.exit_reason {
		ExitReason::Succeed(_) => Ok(info.value),
		other => Err(format!("evm call did not succeed: {other:?}")),
	}
}

/// An account's OWN balance (free + reserved) of `asset` at `at`, read from state. Reserved counts
/// because it is still the account's wealth (a DCA unreserves budget, trades, re-reserves; free
/// alone would misread the shuffle as a gain). Native lives in `System::Account`; other assets in
/// orml `Tokens::Accounts`. An ERC20 aToken reads as absent here — itself the signal a leg moved
/// outside the Substrate balance system (the aToken path in `real_delta` handles those).
pub fn own_balance<T: Transport>(api: &T, account: &[u8; 32], asset: AssetId, at: H256) -> Option<Balance> {
	if asset == NATIVE_ASSET {
		native_own(api, account, at)
	} else {
		token_own(api, account, asset, at)
	}
}

/// Real balance change parent→at from the authoritative ledger for `asset`: ERC20 `balanceOf` for
/// aTokens (EVM-held), else orml/native. `None` if a needed read fails.
pub fn real_delta<T: Transport>(api: &T, account: &[u8; 32], asset: AssetId, parent: H256, at: H256) -> Option<i128> {
	if let Some(contract) = atoken_contract(api, asset, at) {
		let who = evm_h160(account);
		let before = erc20_balance_of(api, contract, who, parent)?;
		let after = erc20_balance_of(api, contract, who, at)?;
		Some(after as i128 - before as i128)
	} else {
		let before = own_balance(api, account, asset, parent)?;
		let after = own_balance(api, account, asset, at)?;
		Some(after as i128 - before as i128)
	}
}

/// The EVM identity an account's aTokens are held under (core's pure classifier, as `H160`).
pub fn evm_h160(account: &[u8; 32]) -> H160 {
	H160(cerebro_core::accounts::evm_address(account))
}

fn token_own<T: Transport>(api: &T, account: &[u8; 32], asset: AssetId, at: H256) -> Option<Balance> {
	let mut key = Vec::with_capacity(80);
	key.extend_from_slice(&twox_128(b"Tokens"));
	key.extend_from_slice(&twox_128(b"Accounts"));
	// orml_tokens::Accounts = DoubleMap<Blake2_128Concat, AccountId, Twox64Concat, CurrencyId>.
	push_blake2_128_concat(&mut key, account);
	push_twox64_concat(&mut key, &asset.encode());
	match api.storage(&key, at.0).ok()? {
		Some(bytes) => Some(read_u128_le(&bytes, 0).saturating_add(read_u128_le(&bytes, 16))),
		None => Some(0),
	}
}

fn native_own<T: Transport>(api: &T, account: &[u8; 32], at: H256) -> Option<Balance> {
	let mut key = Vec::with_capacity(64);
	key.extend_from_slice(&twox_128(b"System"));
	key.extend_from_slice(&twox_128(b"Account"));
	push_blake2_128_concat(&mut key, account);
	match api.storage(&key, at.0).ok()? {
		// AccountInfo: 4×u32 counters, then AccountData { free u128 @16, reserved u128 @32, .. }.
		Some(bytes) => Some(read_u128_le(&bytes, 16).saturating_add(read_u128_le(&bytes, 32))),
		None => Some(0),
	}
}

/// aToken/ERC20 contract for an asset (as the runtime's `BoundErc20` resolves it): the
/// `AccountKey20` from `AssetRegistry::AssetLocations[asset]`. `None` for a plain orml asset.
/// Cached for the process lifetime (locations are immutable post-registration).
pub fn atoken_contract<T: Transport>(api: &T, asset: AssetId, at: H256) -> Option<H160> {
	static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<AssetId, Option<H160>>>> =
		std::sync::OnceLock::new();
	let cache = CACHE.get_or_init(Default::default);
	if let Some(hit) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&asset) {
		return *hit;
	}
	let resolved = resolve_atoken_contract(api, asset, at)?;
	if api.cache_reads() {
		cache.lock().unwrap_or_else(|e| e.into_inner()).insert(asset, resolved);
	}
	resolved
}

/// `Some(None)` = the asset has no `AccountKey20` location; `None` = the read failed.
fn resolve_atoken_contract<T: Transport>(api: &T, asset: AssetId, at: H256) -> Option<Option<H160>> {
	let raw = api.storage(&asset_location_key(asset), at.0).ok()?;
	Some(raw.and_then(|raw| location_account_key20(&raw)))
}

fn asset_location_key(asset: AssetId) -> Vec<u8> {
	let mut key = Vec::with_capacity(48);
	key.extend_from_slice(&twox_128(b"AssetRegistry"));
	key.extend_from_slice(&twox_128(b"AssetLocations"));
	push_blake2_128_concat(&mut key, &asset.encode());
	key
}

/// Per-`RemoteApi` registry caches. Instance-scoped (not process-static) so replay fixtures in one
/// test process stay isolated — a cassette miss in one case must not poison the next.
#[derive(Default)]
pub struct RegistryCache {
	/// ERC20 contract → asset id, from the `AssetLocations` reverse scan.
	contract_to_asset: Option<std::collections::HashMap<H160, AssetId>>,
	/// Underlying asset → its aToken's asset id (via `UNDERLYING_ASSET_ADDRESS()`).
	underlying_to_atoken: Option<std::collections::BTreeMap<AssetId, AssetId>>,
}

/// Resolve an Aave-event `reserve` EVM address to a Hydration asset id: the synthetic orml
/// precompile `0x00…01 <id BE4>`, else the real ERC20 contract via the `AssetLocations` reverse map.
pub fn asset_of_reserve(api: &RemoteApi, reserve: H160, at: H256) -> Option<AssetId> {
	asset_of_reserve_in(&contract_reverse_map(api, at), reserve)
}

/// [`asset_of_reserve`] against an already built `contract → asset` map.
pub fn asset_of_reserve_in(contracts: &std::collections::HashMap<H160, AssetId>, reserve: H160) -> Option<AssetId> {
	let b = reserve.as_bytes();
	if b[..15].iter().all(|&x| x == 0) && b[15] == 1 {
		return Some(u32::from_be_bytes([b[16], b[17], b[18], b[19]]));
	}
	contracts.get(&reserve).copied()
}

fn contract_reverse_map(api: &RemoteApi, at: H256) -> std::collections::HashMap<H160, AssetId> {
	if let Some(m) = &api.registry_cache.borrow().contract_to_asset {
		return m.clone();
	}
	let m = build_contract_map(api, at);
	api.registry_cache.borrow_mut().contract_to_asset = Some(m.clone());
	m
}

/// ERC20 contract → asset id for every asset whose `AssetLocations` entry is an `AccountKey20`.
pub fn build_contract_map<T: Transport>(api: &T, at: H256) -> std::collections::HashMap<H160, AssetId> {
	let mut m = std::collections::HashMap::new();
	let mut prefix = Vec::with_capacity(32);
	prefix.extend_from_slice(&twox_128(b"AssetRegistry"));
	prefix.extend_from_slice(&twox_128(b"AssetLocations"));
	let keys = api.storage_keys(&prefix, at.0).unwrap_or_default();
	let keys: Vec<Vec<u8>> = keys.into_iter().filter(|k| k.len() >= 4).collect();
	let values = api.storage_many(&keys, at.0).unwrap_or_default();
	for (key, raw) in keys.iter().zip(values) {
		let idb: [u8; 4] = key[key.len() - 4..].try_into().expect("filtered to len >= 4; qed");
		if let Some(h) = raw.and_then(|raw| location_account_key20(&raw)) {
			m.insert(h, u32::from_le_bytes(idb));
		}
	}
	m
}

/// Underlying asset id → its aToken's asset id, for every registered ERC20 that answers the Aave
/// `UNDERLYING_ASSET_ADDRESS()` getter. Lets confirmation SWEEP a supply/withdraw's readable
/// aToken counter-leg instead of folding it (folding a swept counter-leg double-counts). Contracts
/// are probed in ascending asset-id order and the first claim on an underlying wins, so the result
/// is deterministic. Errors (revert, cassette miss) skip the contract — callers then keep the fold
/// fallback, which is the pre-existing behavior.
pub fn underlying_to_atoken(api: &RemoteApi, at: H256) -> std::collections::BTreeMap<AssetId, AssetId> {
	if let Some(m) = &api.registry_cache.borrow().underlying_to_atoken {
		return m.clone();
	}
	let m = build_underlying_to_atoken(api, &contract_reverse_map(api, at), at);
	api.registry_cache.borrow_mut().underlying_to_atoken = Some(m.clone());
	m
}

/// [`underlying_to_atoken`] without the `RemoteApi` cache, from an already built contract map.
pub fn build_underlying_to_atoken<T: Transport>(
	api: &T,
	contracts: &std::collections::HashMap<H160, AssetId>,
	at: H256,
) -> std::collections::BTreeMap<AssetId, AssetId> {
	// asset-id-sorted contracts for deterministic conflict resolution
	let by_asset: std::collections::BTreeMap<AssetId, H160> = contracts.iter().map(|(c, a)| (*a, *c)).collect();
	let mut m = std::collections::BTreeMap::new();
	for (&atoken_asset, &contract) in &by_asset {
		let Some(out) = eth_call_bytes(api, contract, UNDERLYING_ASSET_ADDRESS.to_vec(), at) else {
			continue;
		};
		if out.len() < 32 {
			continue;
		}
		let underlying = H160::from_slice(&out[12..32]);
		if underlying.is_zero() {
			continue;
		}
		if let Some(u) = asset_of_reserve_in(contracts, underlying) {
			if u != atoken_asset {
				m.entry(u).or_insert(atoken_asset);
			}
		}
	}
	m
}

/// Extract `AccountKey20` from a SCALE `AssetLocation` whose interior is `X1(AccountKey20{None,key})`.
/// Layout: parents u8 · interior tag (X1=1) · junction tag (AccountKey20=3) · network None=0 · key[20].
fn location_account_key20(bytes: &[u8]) -> Option<H160> {
	if bytes.len() >= 24 && bytes[1] == 1 && bytes[2] == 3 && bytes[3] == 0 {
		Some(H160::from_slice(&bytes[4..24]))
	} else {
		None
	}
}

pub(crate) fn push_blake2_128_concat(key: &mut Vec<u8>, raw: &[u8]) {
	key.extend_from_slice(&blake2_128(raw));
	key.extend_from_slice(raw);
}
pub(crate) fn push_twox64_concat(key: &mut Vec<u8>, raw: &[u8]) {
	key.extend_from_slice(&twox_64(raw));
	key.extend_from_slice(raw);
}
pub(crate) fn read_u128_le(bytes: &[u8], offset: usize) -> u128 {
	bytes
		.get(offset..offset + 16)
		.map(|s| u128::from_le_bytes(s.try_into().expect("16-byte slice; qed")))
		.unwrap_or(0)
}
