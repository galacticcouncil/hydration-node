// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! `StorageOverride` that augments Frontier's reads with synthetic ethereum txs,
//! produced client-side from a block's events via
//! `event_logs::synthetic_txs_from_records` (no runtime API, any runtime version).
//!
//! Real entries come first, synth appended in stable order, so a synth tx's index
//! resolves consistently across `current_transaction_statuses`/`current_receipts`
//! and `current_block.transactions` (fc-rpc indexes `block.transactions[index]`).
//! The block header is left canonical (hash unchanged); `eth_getLogs` discovery of
//! synth-only blocks is handled by the sibling `eth_filter` module.

use std::{
	collections::HashMap,
	marker::PhantomData,
	num::NonZeroUsize,
	sync::{
		atomic::{AtomicUsize, Ordering},
		Arc, Mutex, Once,
	},
};

use super::compat_events;
use super::metadata_events::{self, EventLayout, Missing, Verdict};
use codec::Decode;
use fc_rpc::StorageOverride;
use fp_rpc::TransactionStatus;
use hydradx_runtime::evm::event_logs::synthetic_txs_from_records;
use lru::LruCache;
use pallet_ethereum::{Block as EthBlock, Receipt as EthReceipt, Transaction as EthTransaction};
use primitives::Block;
use sc_client_api::{backend::Backend, BlockBackend, StorageProvider};
use sp_api::{Metadata as MetadataApi, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_core::{hashing::twox_128, H160, H256, U256};
use sp_runtime::traits::{BlakeTwo256, Block as BlockT, Hash as HashT, Header as HeaderT};
use sp_storage::StorageKey;

type Hash = <Block as BlockT>::Hash;
type SynthTxs = Vec<(EthTransaction, TransactionStatus, EthReceipt)>;

/// v15 carries the same type registry as v14 and is what every runtime since 2023 emits;
/// `metadata()` (v14) is the fallback and is guaranteed to exist.
const METADATA_VERSION: u32 = 15;

// A recoverable skew is one persistent condition, so it is reported once. Data loss is
// not: every lossy outcome gets its OWN counter, because sharing a latch with a
// recoverable case silently swallowed real loss — a `Compat` block consumed the latch and
// every later dropped record went unreported.
static SKEW_WARNED: Once = Once::new();
static NO_METADATA_WARNED: Once = Once::new();
static DROPPED_SEEN: AtomicUsize = AtomicUsize::new(0);
static PARTIAL_SEEN: AtomicUsize = AtomicUsize::new(0);
static DECODE_FAILED: Once = Once::new();

fn names(missing: &[Missing]) -> String {
	missing.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ")
}

/// Which block's state to read the metadata out of, to describe the runtime that *executed*
/// `at`.
///
/// The parent's, not `at`'s. A runtime api call at `at` runs whatever `:code` sits in `at`'s
/// post-state, and on the block where an upgrade is applied cumulus has already written the
/// NEW code there while the block itself was executed by the OLD one. Reading metadata at
/// `at` would then cache the new runtime's layout under the old runtime's spec version and
/// mis-decode every later old-spec block. The parent's post-state holds the code that
/// actually ran, in every case:
///
/// ```text
/// block N-1 ─┬─ :code = old
/// block N    ├─ executed by old; on_initialize writes :code = new; LastRuntimeUpgrade = old
/// block N+1  └─ executed by new; LastRuntimeUpgrade = new
///
///   layout for N   → spec old (from N)   + metadata from N-1 (old code)   ✓
///   layout for N+1 → spec new (from N+1) + metadata from N   (new code)   ✓
/// ```
///
/// Falls back to `at` at genesis, or when the parent is unknown — no worse than before, and
/// the odds of the oldest block this node retains being an upgrade block are negligible.
fn metadata_source<C>(client: &C, at: Hash) -> Hash
where
	C: HeaderBackend<Block>,
{
	client
		.header(at)
		.ok()
		.flatten()
		.map(|header| *header.parent_hash())
		.filter(|parent| *parent != Hash::default())
		.unwrap_or(at)
}

/// Log the first occurrence, then back off by powers of two, so persistent loss stays
/// visible without flooding one line per block.
fn should_report(counter: &AtomicUsize) -> usize {
	let n = counter.fetch_add(1, Ordering::Relaxed);
	if n == 0 || n.is_power_of_two() {
		n + 1
	} else {
		0
	}
}

// `synthetic()` is invoked once each by `current_block`/`current_receipts`/
// `current_transaction_statuses`, so a range `eth_getLogs` would re-read and
// re-translate a block's events 3× without this per-block cache.
const SYNTH_CACHE_CAP: usize = 256;

pub struct SyntheticStorageOverride<C, BE> {
	inner: Arc<dyn StorageOverride<Block>>,
	client: Arc<C>,
	cache: Mutex<LruCache<Hash, Arc<SynthTxs>>>,
	// One entry per runtime version, not per block: reading metadata means executing wasm,
	// and a spec version's event layout is the same at every block that runs it. `None`
	// records that it could not be built, so a pruned or unknown runtime is not retried
	// on every read.
	layouts: Mutex<HashMap<u32, Option<Arc<EventLayout>>>>,
	_marker: PhantomData<BE>,
}

impl<C, BE> SyntheticStorageOverride<C, BE> {
	pub fn new(inner: Arc<dyn StorageOverride<Block>>, client: Arc<C>) -> Self {
		Self {
			inner,
			client,
			cache: Mutex::new(LruCache::new(
				NonZeroUsize::new(SYNTH_CACHE_CAP).expect("non-zero; qed"),
			)),
			layouts: Mutex::new(HashMap::new()),
			_marker: PhantomData,
		}
	}
}

fn storage_key(pallet: &[u8], item: &[u8]) -> StorageKey {
	StorageKey([twox_128(pallet), twox_128(item)].concat())
}

impl<C, BE> SyntheticStorageOverride<C, BE>
where
	C: StorageProvider<Block, BE>
		+ HeaderBackend<Block>
		+ BlockBackend<Block>
		+ ProvideRuntimeApi<Block>
		+ Send
		+ Sync
		+ 'static,
	C::Api: MetadataApi<Block>,
	BE: Backend<Block> + Send + Sync + 'static,
{
	/// The chain's own description of how it encodes events at `at`.
	///
	/// Keyed by spec version, so the wasm call happens once per runtime rather than once per
	/// block. `None` means the chain could not be asked — an unknown metadata version, or
	/// state this node no longer has — and the caller falls back to compiled types.
	fn layout(&self, at: Hash) -> Option<Arc<EventLayout>> {
		let spec = self.spec_version(at);
		// The build is held under the lock rather than raced: two rpc threads reaching an
		// unseen runtime at once would otherwise each execute the metadata call and each log
		// the summary. Waiting costs one wasm call's worth of latency, once per runtime.
		let mut layouts = self.layouts.lock().expect("layout cache mutex; qed");
		if let Some(known) = layouts.get(&spec) {
			return known.clone();
		}
		let layout = self.build_layout(metadata_source(self.client.as_ref(), at), spec);
		layouts.insert(spec, layout.clone());
		layout
	}

	/// The spec version of the runtime that *executed* `at`, out of state rather than the
	/// runtime api: no wasm call, and `frame_executive` only bumps
	/// `System::LastRuntimeUpgrade` when a new runtime runs for the first time, so the value
	/// in a block's post-state names the runtime that produced that block's events — which is
	/// exactly the layout needed to decode them. Only the leading `Compact<u32>` of
	/// `LastRuntimeUpgradeInfo` is read, so the `spec_name` type moving between sdks cannot
	/// break it. A missing entry just shares one cache slot.
	fn spec_version(&self, at: Hash) -> u32 {
		self.client
			.storage(at, &storage_key(b"System", b"LastRuntimeUpgrade"))
			.ok()
			.flatten()
			.and_then(|data| codec::Compact::<u32>::decode(&mut &data.0[..]).ok())
			.map(|v| v.0)
			.unwrap_or_default()
	}

	fn build_layout(&self, at: Hash, spec: u32) -> Option<Arc<EventLayout>> {
		let api = self.client.runtime_api();
		let raw = api
			.metadata_at_version(at, METADATA_VERSION)
			.ok()
			.flatten()
			.or_else(|| api.metadata(at).ok())?;
		let layout = match EventLayout::new(&raw) {
			Ok(layout) => layout,
			Err(fault) => {
				log::warn!(
					target: "synthetic-logs",
					"could not read runtime {spec}'s event layout out of its metadata ({fault:?}); \
					 falling back to this node's compiled types, which are only correct while its \
					 polkadot-sdk matches the runtime's."
				);
				return None;
			}
		};
		// independent of any layout skew: an event synth looks for that this runtime does not
		// raise at all means the logs derived from it just stop appearing.
		let absent = layout.synth_events_absent();
		if !absent.is_empty() {
			log::warn!(
				target: "synthetic-logs",
				"runtime {spec} does not raise {} event(s) synth reads, so nothing will be derived \
				 from them: {}. either the runtime renamed them or synth needs updating.",
				absent.len(),
				absent.join(", ")
			);
		}
		match layout.verdict() {
			Verdict::Identical => log::info!(
				target: "synthetic-logs",
				"runtime {spec} encodes events exactly as this node's types do; synth logs decode natively."
			),
			Verdict::Divergent => {
				log::info!(
					target: "synthetic-logs",
					"runtime {spec} encodes events differently from this node's compiled types; synth \
					 logs are transcoded from its metadata."
				);
				// the only events that can still be lost, known before a single block is read.
				// split by whether synth reads them: a newer runtime's new event costs nothing,
				// one of `SYNTH_EVENTS` going missing changes what eth_getLogs returns.
				let (harmful, harmless): (Vec<_>, Vec<_>) =
					layout.unrepresentable().into_iter().partition(|m| m.costs_logs);
				if !harmless.is_empty() {
					log::info!(
						target: "synthetic-logs",
						"runtime {spec} has {} event(s) this node has no type for; none feed synth, so \
						 no log is affected: {}",
						harmless.len(),
						names(&harmless)
					);
				}
				if !harmful.is_empty() {
					log::error!(
						target: "synthetic-logs",
						"runtime {spec} has {} event(s) this node has no type for that synth DOES read, \
						 so every block raising one loses a log: {}. deploy a node built against the \
						 live runtime's sdk.",
						harmful.len(),
						names(&harmful)
					);
				}
			}
		}
		Some(Arc::new(layout))
	}

	fn read_decode<T: Decode>(&self, at: Hash, key: &StorageKey) -> Option<T> {
		let data = self.client.storage(at, key).ok().flatten()?;
		Decode::decode(&mut &data.0[..]).ok()
	}

	fn synthetic(&self, at: Hash) -> SynthTxs {
		if let Some(hit) = self.cache.lock().expect("synth cache mutex; qed").get(&at) {
			return (**hit).clone();
		}
		let txs = Arc::new(self.compute_synthetic(at));
		self.cache.lock().expect("synth cache mutex; qed").put(at, txs.clone());
		(*txs).clone()
	}

	fn compute_synthetic(&self, at: Hash) -> SynthTxs {
		let raw = match self.client.storage(at, &storage_key(b"System", b"Events")) {
			Ok(Some(data)) => data.0,
			_ => return Vec::new(),
		};
		let layout = self.layout(at);
		let records = match compat_events::read_events(&raw, layout.as_deref()) {
			Some((records, compat_events::Source::Verified)) => records,
			Some((
				records,
				compat_events::Source::Metadata {
					dropped,
					recovered,
					trailing,
				},
			)) => {
				// The only loss the metadata path can suffer: an event whose pallet or variant
				// this node's `RuntimeEvent` does not contain. It is named, and it cost only
				// itself — the record was measured against the chain's own metadata before
				// being stepped over. Whether that matters depends entirely on whether synth
				// reads the event, so only that case is an error; the rest is already listed
				// once per runtime version and would otherwise be one false alarm per block.
				let costly = dropped
					.iter()
					.filter(|(name, _)| metadata_events::feeds_synth(name))
					.map(|(name, fault)| format!("{name} ({fault:?})"))
					.collect::<Vec<_>>();
				if !costly.is_empty() || trailing > 0 {
					let seen = should_report(&DROPPED_SEEN);
					if seen > 0 {
						log::error!(
							target: "synthetic-logs",
							"synth logs are INCOMPLETE for block {at:?}: this node has no type for \
							 {} event(s) synth reads [{}], {trailing} trailing byte(s), {recovered} \
							 EVM.Log salvaged. {seen} affected block(s) so far. deploy a node built \
							 against the live runtime's sdk.",
							costly.len(),
							costly.join(", ")
						)
					}
				} else if !dropped.is_empty() {
					log::debug!(
						target: "synthetic-logs",
						"block {at:?}: stepped over {} event(s) this node has no type for; none feed \
						 synth, so every log is present",
						dropped.len()
					);
				}
				records
			}
			Some((records, compat_events::Source::Native)) => {
				NO_METADATA_WARNED.call_once(|| {
					log::warn!(
						target: "synthetic-logs",
						"no usable event layout from the chain's metadata, so synth logs are decoded with \
						 this node's compiled types. that is only correct while the node's polkadot-sdk \
						 matches the on-chain runtime's."
					)
				});
				records
			}
			Some((records, compat_events::Source::Compat)) => {
				SKEW_WARNED.call_once(|| {
					log::warn!(
						target: "synthetic-logs",
						"System::Events read through a compat balances layout: this node's polkadot-sdk \
						 differs from the on-chain runtime's. synth logs are being recovered, but deploy a \
						 node built against the live runtime's sdk."
					)
				});
				records
			}
			Some((
				records,
				compat_events::Source::Partial {
					skipped,
					recovered,
					trailing,
				},
			)) => {
				// Real data loss: a dropped record takes its logs with it, and a resync can skip a
				// RUN of records (observed: 12 of 19 logs on one block, including a wormhole
				// LogMessagePublished). Never share a latch with the recoverable Compat case.
				let seen = should_report(&PARTIAL_SEEN);
				if seen > 0 {
					log::error!(
						target: "synthetic-logs",
						"System::Events only partially decodable — synth logs are INCOMPLETE for \
						 block {at:?}: dropped {skipped} event(s), salvaged {recovered} EVM.Log, \
						 {trailing} trailing byte(s). {seen} affected block(s) so far. this node's \
						 RuntimeEvent does not match the on-chain runtime's; deploy a node built \
						 against the live runtime's sdk."
					)
				}
				records
			}
			// never silent: an undecodable block otherwise looks event-free and drops every
			// synth log in it — that is how v49.2.0 hid wormhole LogMessagePublished.
			None => {
				DECODE_FAILED.call_once(|| {
					log::error!(
						target: "synthetic-logs",
						"cannot decode System::Events with any known layout — synthetic eth logs are \
						 INCOMPLETE. this node's RuntimeEvent does not match the on-chain runtime's; see \
						 node/src/synthetic_logs/compat_events.rs."
					)
				});
				log::debug!(target: "synthetic-logs", "System::Events undecodable at {at:?}");
				return Vec::new();
			}
		};
		if records.is_empty() {
			return Vec::new();
		}
		let chain_id: u64 = self
			.read_decode(at, &storage_key(b"EVMChainId", b"ChainId"))
			.unwrap_or_default();
		let real_statuses = self.inner.current_transaction_statuses(at).unwrap_or_default();
		// The extrinsic hashes go into each synth tx's `input` so an indexer can join it
		// back to its extrinsic. `Block::Extrinsic` is `OpaqueExtrinsic`, so this hashes
		// raw bytes and decodes no `RuntimeCall` — it cannot break on the sdk skew that
		// `compat_events` exists for. An unavailable body degrades to the zero hash,
		// which is still unique because `at` is folded in.
		let extrinsic_hashes: Vec<[u8; 32]> = self
			.client
			.block_body(at)
			.ok()
			.flatten()
			.map(|body| body.iter().map(|xt| BlakeTwo256::hash_of(xt).0).collect())
			.unwrap_or_default();

		// Only used to order the synth `nonce`; identity comes from `at` + extrinsic hash.
		let block_number: u64 = self.client.number(at).ok().flatten().map_or(0, u64::from);

		// `at` is the block's OWN hash: sibling blocks share a parent, so a parent-based
		// domain gave colliding synth tx hashes on every fork.
		synthetic_txs_from_records(
			&records,
			chain_id,
			at.as_ref(),
			&extrinsic_hashes,
			block_number,
			&real_statuses,
		)
	}
}

impl<C, BE> StorageOverride<Block> for SyntheticStorageOverride<C, BE>
where
	C: StorageProvider<Block, BE>
		+ HeaderBackend<Block>
		+ BlockBackend<Block>
		+ ProvideRuntimeApi<Block>
		+ Send
		+ Sync
		+ 'static,
	C::Api: MetadataApi<Block>,
	BE: Backend<Block> + Send + Sync + 'static,
{
	fn account_code_at(&self, at: Hash, address: H160) -> Option<Vec<u8>> {
		self.inner.account_code_at(at, address)
	}

	fn account_storage_at(&self, at: Hash, address: H160, index: U256) -> Option<H256> {
		self.inner.account_storage_at(at, address, index)
	}

	fn current_block(&self, at: Hash) -> Option<EthBlock> {
		let mut block = self.inner.current_block(at)?;
		// Append synth txs so `eth_getTransactionByHash`/`*_receipt` can index them
		// (fc-rpc does `block.transactions[index]`). The header is left UNTOUCHED so
		// the canonical eth block hash is preserved — surfacing synth logs in
		// `eth_getLogs` is handled by the `eth_filter` module, not by mutating the
		// header bloom (which would change the block hash).
		for (tx, _, _) in self.synthetic(at) {
			block.transactions.push(tx);
		}
		Some(block)
	}

	fn current_receipts(&self, at: Hash) -> Option<Vec<EthReceipt>> {
		let synth = self.synthetic(at);
		match self.inner.current_receipts(at) {
			Some(mut real) => {
				real.extend(synth.into_iter().map(|(_, _, r)| r));
				Some(real)
			}
			None if !synth.is_empty() => Some(synth.into_iter().map(|(_, _, r)| r).collect()),
			None => None,
		}
	}

	fn current_transaction_statuses(&self, at: Hash) -> Option<Vec<TransactionStatus>> {
		let synth = self.synthetic(at);
		match self.inner.current_transaction_statuses(at) {
			Some(mut real) => {
				real.extend(synth.into_iter().map(|(_, s, _)| s));
				Some(real)
			}
			None if !synth.is_empty() => Some(synth.into_iter().map(|(_, s, _)| s).collect()),
			None => None,
		}
	}

	fn elasticity(&self, at: Hash) -> Option<sp_runtime::Permill> {
		self.inner.elasticity(at)
	}

	fn is_eip1559(&self, at: Hash) -> bool {
		self.inner.is_eip1559(at)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_runtime::generic::Header as GenericHeader;
	use sp_runtime::traits::BlakeTwo256 as Hashing;

	type Header = GenericHeader<u32, Hashing>;

	/// Just enough of a header backend to answer "what is this block's parent".
	struct Headers(Vec<(Hash, Hash)>);

	impl HeaderBackend<Block> for Headers {
		fn header(&self, hash: Hash) -> sp_blockchain::Result<Option<Header>> {
			Ok(self.0.iter().find(|(h, _)| *h == hash).map(|(_, parent)| Header {
				parent_hash: *parent,
				number: 1,
				state_root: Default::default(),
				extrinsics_root: Default::default(),
				digest: Default::default(),
			}))
		}
		fn info(&self) -> sp_blockchain::Info<Block> {
			unimplemented!("not read by metadata_source")
		}
		fn status(&self, _: Hash) -> sp_blockchain::Result<sp_blockchain::BlockStatus> {
			unimplemented!("not read by metadata_source")
		}
		fn number(&self, _: Hash) -> sp_blockchain::Result<Option<u32>> {
			unimplemented!("not read by metadata_source")
		}
		fn hash(&self, _: u32) -> sp_blockchain::Result<Option<Hash>> {
			unimplemented!("not read by metadata_source")
		}
	}

	/// The upgrade-block regression: metadata has to come from the parent's state, because a
	/// runtime api call at the upgrade block itself would run the NEW `:code` cumulus has
	/// already written there, and cache the new layout under the old runtime's spec version.
	#[test]
	fn metadata_comes_from_the_parent_state() {
		let (parent, upgrade) = (Hash::repeat_byte(1), Hash::repeat_byte(2));
		let headers = Headers(vec![(upgrade, parent)]);
		assert_eq!(
			metadata_source(&headers, upgrade),
			parent,
			"a block's events were produced by the runtime in its PARENT's post-state"
		);
	}

	/// Genesis has no parent, and an unknown block cannot offer one: fall back to the block
	/// itself rather than reading state at the zero hash.
	#[test]
	fn genesis_and_unknown_blocks_fall_back_to_themselves() {
		let genesis = Hash::repeat_byte(3);
		let headers = Headers(vec![(genesis, Hash::default())]);
		assert_eq!(metadata_source(&headers, genesis), genesis, "genesis has no parent");

		let orphan = Hash::repeat_byte(9);
		assert_eq!(
			metadata_source(&Headers(Vec::new()), orphan),
			orphan,
			"an unknown header must not send us to the zero hash"
		);
	}
}
