// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Event → synthetic-eth-tx translation for the node-indexing variant.
//!
//! Pure translation: `synthetic_txs_from_records` turns a block's
//! `EventRecord`s into synthetic `(Transaction, TransactionStatus, Receipt)`
//! triples using only pure helpers (`account_to_evm_address`,
//! `asset_evm_address`, the `build_*`/`assemble_synth_txs` primitives) — no
//! runtime-state reads. That's what lets the SAME code serve two callers:
//!
//! - `SyntheticEthLogsApi` (this runtime), gathering events from
//!   `frame_system::Events` — fast path for the current+ runtime; and
//! - the node's client-side indexer, which reads `System::Events` from state
//!   and calls `synthetic_txs_from_records` directly — works against ANY
//!   runtime version (it never invokes a runtime API), so blocks produced
//!   before this runtime shipped are still indexed (bounded only by whether
//!   their events still decode).
//!
//! v1 scope: token `Transfer` (orml-tokens + native HDX via pallet-balances),
//! `Swapped3` → uniswap-v2 `Swap`, and `pallet_evm::Event::Log` from internal
//! `Executor::call` paths (deduped against real eth txs).

use crate::{Runtime, RuntimeEvent};
use frame_support::traits::Get;
use frame_system::{EventRecord, Phase};
use pallet_broadcast::types::{Asset, ExecutionType, TradeOperation};
use pallet_ethereum::{Receipt, Transaction, TransactionStatus};
use pallet_synthetic_logs::{
	account_to_evm_address, assemble_synth_txs, asset_evm_address, build_erc20_transfer_log, build_uniswap_v2_swap_log,
	Bucket, HookPhase,
};
use primitive_types::{H160, U256};
use primitives::constants::chain::CORE_ASSET_ID;
use primitives::AccountId;
use sp_core::H256;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec::Vec;

sp_api::decl_runtime_apis! {
	/// Synthetic ethereum tx records derived from a block's substrate events,
	/// for the node's eth-rpc indexing layer. Read-only; no consensus state.
	pub trait SyntheticEthLogsApi {
		/// `(transaction, status, receipt)` triples for the block at which this
		/// API is invoked, assembled from `frame_system::Events`. Indices
		/// continue after the block's real eth txs.
		fn synthetic_transactions() -> Vec<(Transaction, TransactionStatus, Receipt)>;
	}
}

fn evm_addr(account: &AccountId) -> H160 {
	account_to_evm_address(account.as_ref())
}

// Pure mirror of `erc20_mapping::is_asset_address`: prefix `0x..01` (15 zero
// bytes then `1`), the range `asset_evm_address` produces.
fn is_asset_address(addr: H160) -> bool {
	addr.0[..15] == [0u8; 15] && addr.0[15] == 1
}

/// Pure: evm logs an indexer should see for one runtime event.
pub fn logs_from_event(event: &RuntimeEvent) -> Vec<(H160, ethereum::Log)> {
	match event {
		RuntimeEvent::Tokens(orml_tokens::Event::Transfer {
			currency_id,
			from,
			to,
			amount,
		}) => {
			if *amount == 0 {
				return Vec::new();
			}
			let addr = asset_evm_address(*currency_id);
			sp_std::vec![(
				addr,
				build_erc20_transfer_log(addr, evm_addr(from), evm_addr(to), U256::from(*amount))
			)]
		}
		RuntimeEvent::Balances(pallet_balances::Event::Transfer { from, to, amount }) => {
			if *amount == 0 {
				return Vec::new();
			}
			let addr = asset_evm_address(CORE_ASSET_ID);
			sp_std::vec![(
				addr,
				build_erc20_transfer_log(addr, evm_addr(from), evm_addr(to), U256::from(*amount))
			)]
		}
		RuntimeEvent::Broadcast(pallet_broadcast::Event::Swapped3 {
			swapper,
			filler,
			operation,
			inputs,
			outputs,
			..
		}) => swap_log(swapper, filler, operation, inputs, outputs)
			.into_iter()
			.collect(),
		// EVM logs from internal `Executor::call` paths (hsm, dispatcher-driven,
		// liquidations); deduped against real eth txs by the caller.
		RuntimeEvent::EVM(pallet_evm::Event::Log { log }) => {
			sp_std::vec![(
				log.address,
				ethereum::Log {
					address: log.address,
					topics: log.topics.clone(),
					data: log.data.clone(),
				}
			)]
		}
		_ => Vec::new(),
	}
}

/// Pure: a trade → uniswap-v2 `Swap` log (emitter = pool's evm address), or
/// `None` for trades outside v1 scope (bilateral `ExactIn`/`ExactOut`).
fn swap_log(
	swapper: &AccountId,
	filler: &AccountId,
	operation: &TradeOperation,
	inputs: &[Asset],
	outputs: &[Asset],
) -> Option<(H160, ethereum::Log)> {
	if !matches!(operation, TradeOperation::ExactIn | TradeOperation::ExactOut) {
		return None;
	}
	if inputs.len() != 1 || outputs.len() != 1 {
		return None;
	}
	let in_asset = inputs[0].asset;
	let in_amount = inputs[0].amount;
	let out_asset = outputs[0].asset;
	let out_amount = outputs[0].amount;
	if in_amount == 0 && out_amount == 0 {
		return None;
	}

	let pool_address = evm_addr(filler);
	if is_asset_address(pool_address) {
		// would collide with the erc20 asset-address range; skip (~2^-128).
		return None;
	}

	let sender = evm_addr(swapper);
	let input_is_token0 = in_asset <= out_asset; // token0 = lower asset id
	let log = build_uniswap_v2_swap_log(
		pool_address,
		sender,
		sender,
		input_is_token0,
		U256::from(in_amount),
		U256::from(out_amount),
	);
	Some((pool_address, log))
}

/// Bucket origin for hook-phase events. Only `Swapped3` carries its originating
/// context (`operation_stack`); others fall back to `None`.
fn event_origin_hint(event: &RuntimeEvent) -> Option<ExecutionType> {
	match event {
		RuntimeEvent::Broadcast(pallet_broadcast::Event::Swapped3 { operation_stack, .. }) => {
			operation_stack.first().copied()
		}
		_ => None,
	}
}

/// PURE: assemble a block's synthetic txs from its event records. Shared by the
/// runtime API and the node's client-side indexer (which passes records read
/// from state + the block's `chain_id`/`parent_hash`/`number` and the count of
/// real eth txs as `base_tx_index`).
pub fn synthetic_txs_from_records(
	records: &[EventRecord<RuntimeEvent, H256>],
	chain_id: u64,
	parent_hash: &[u8],
	block_number: u64,
	base_tx_index: u32,
) -> Vec<(Transaction, TransactionStatus, Receipt)> {
	// Extrinsics that ran a real eth tx: their EVM logs are already in that tx's
	// receipt, so skip re-synthesizing `pallet_evm::Event::Log` for them.
	let mut evm_tx_extrinsics: BTreeSet<u32> = BTreeSet::new();
	for rec in records.iter() {
		if matches!(
			rec.event,
			RuntimeEvent::Ethereum(pallet_ethereum::Event::Executed { .. })
		) {
			if let Phase::ApplyExtrinsic(i) = rec.phase {
				evm_tx_extrinsics.insert(i);
			}
		}
	}

	let mut entries: Vec<(Bucket, H160, ethereum::Log)> = Vec::new();
	for rec in records.iter() {
		if matches!(rec.event, RuntimeEvent::EVM(pallet_evm::Event::Log { .. })) {
			if let Phase::ApplyExtrinsic(i) = rec.phase {
				if evm_tx_extrinsics.contains(&i) {
					continue;
				}
			}
		}
		let bucket = match rec.phase {
			Phase::ApplyExtrinsic(i) => Bucket::Extrinsic(i),
			Phase::Initialization => Bucket::Hook {
				phase: HookPhase::Initialization,
				origin: event_origin_hint(&rec.event),
			},
			Phase::Finalization => Bucket::Hook {
				phase: HookPhase::Finalization,
				origin: event_origin_hint(&rec.event),
			},
		};
		for (emitter, log) in logs_from_event(&rec.event) {
			entries.push((bucket, emitter, log));
		}
	}

	if entries.is_empty() {
		return Vec::new();
	}
	assemble_synth_txs(entries, chain_id, parent_hash, block_number, base_tx_index)
}

/// Runtime-API impl: gather this block's events + context from state, then
/// delegate to the pure `synthetic_txs_from_records`.
pub fn synthetic_transactions() -> Vec<(Transaction, TransactionStatus, Receipt)> {
	let records: Vec<_> = frame_system::Pallet::<Runtime>::read_events_no_consensus()
		.map(|boxed| *boxed)
		.collect();
	let chain_id = <Runtime as pallet_evm::Config>::ChainId::get();
	let parent_hash = frame_system::Pallet::<Runtime>::parent_hash();
	let block_number =
		UniqueSaturatedInto::<u64>::unique_saturated_into(frame_system::Pallet::<Runtime>::block_number());
	let base_tx_index = pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
		.map(|s| s.len() as u32)
		.unwrap_or(0);
	synthetic_txs_from_records(&records, chain_id, parent_hash.as_ref(), block_number, base_tx_index)
}
