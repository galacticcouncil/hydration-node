// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Event → evm-log translation for the node-indexing variant.
//!
//! Reads a `RuntimeEvent` and returns the evm logs an indexer should see. This
//! is the read-only counterpart to the mutation-hook translators: the runtime
//! API (and, behind it, the node's Frontier indexing layer) iterates
//! `frame_system::Events` and feeds each one here, instead of logs being
//! produced as a side effect of balance mutations.
//!
//! It reuses the exact builders the hooks use (`erc20_logs::transfer_log`,
//! `swap_logs::swap_log`), so event-derived logs are byte-identical to the
//! on-chain synth-tx logs — that equivalence is what preserves evm-client
//! compatibility across the two delivery paths.
//!
//! v1 scope: token `Transfer` (orml-tokens + native HDX via pallet-balances)
//! and `Swapped3` → uniswap-v2 `Swap`. Mint/burn/reserve/slash are deferred:
//! unlike mutation hooks, the event stream can't always disambiguate them
//! (e.g. balances `Slashed` doesn't say free vs reserved, and mint can surface
//! as either `Minted` or `Deposit`), so the canonical event per movement must
//! be pinned down before adding them to avoid double-counting.
//!
//! Reading native HDX as a plain `pallet_balances::Event::Transfer` is exactly
//! what lets this variant drop the `BalancesHooks` SDK fork.

use crate::evm::{erc20_logs, swap_logs};
use crate::{Runtime, RuntimeEvent};
use frame_support::traits::Get;
use frame_system::Phase;
use hydradx_traits::evm::InspectEvmAccounts;
use pallet_broadcast::types::ExecutionType;
use pallet_ethereum::{Receipt, Transaction, TransactionStatus};
use pallet_synthetic_logs::{assemble_synth_txs, Bucket, HookPhase};
use primitive_types::H160;
use primitives::constants::chain::CORE_ASSET_ID;
use primitives::AccountId;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec::Vec;

sp_api::decl_runtime_apis! {
	/// Synthetic ethereum tx records derived from a block's substrate events
	/// (token transfers + `Swapped3` swaps), for the node's eth-rpc indexing
	/// layer. Read-only; consumes no consensus state.
	pub trait SyntheticEthLogsApi {
		/// `(transaction, status, receipt)` triples for the block at which this
		/// API is invoked, assembled from `frame_system::Events`. Excludes
		/// events emitted inside ethereum transactions (those logs are already
		/// in the real eth tx). Indices continue after the block's real eth txs.
		fn synthetic_transactions() -> Vec<(Transaction, TransactionStatus, Receipt)>;
	}
}

fn evm_addr(account: &AccountId) -> H160 {
	pallet_evm_accounts::Pallet::<Runtime>::evm_address(account)
}

/// Logs an indexer should see for a single runtime event.
///
/// The caller decides *which* events to feed: events emitted inside an
/// ethereum transaction are skipped upstream, since the real eth tx already
/// carries their logs (the inline precompile emission). This function only
/// translates; it does not dedup.
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
			sp_std::vec![erc20_logs::transfer_log(
				*currency_id,
				evm_addr(from),
				evm_addr(to),
				*amount
			)]
		}
		RuntimeEvent::Balances(pallet_balances::Event::Transfer { from, to, amount }) => {
			if *amount == 0 {
				return Vec::new();
			}
			sp_std::vec![erc20_logs::transfer_log(
				CORE_ASSET_ID,
				evm_addr(from),
				evm_addr(to),
				*amount,
			)]
		}
		RuntimeEvent::Broadcast(pallet_broadcast::Event::Swapped3 {
			swapper,
			filler,
			operation,
			inputs,
			outputs,
			..
		}) => swap_logs::swap_log(swapper, filler, operation, inputs, outputs)
			.into_iter()
			.collect(),
		_ => Vec::new(),
	}
}

/// Bucket origin for hook-phase events. Only `Swapped3` carries its
/// originating context (`operation_stack`) in the event itself, so swaps in
/// `on_initialize`/`on_finalize` (e.g. DCA) recover their per-schedule
/// attribution; other hook-phase events fall back to `None` (coarser grouping
/// than the on-chain hook path, which read the live broadcast context).
fn event_origin_hint(event: &RuntimeEvent) -> Option<ExecutionType> {
	match event {
		RuntimeEvent::Broadcast(pallet_broadcast::Event::Swapped3 { operation_stack, .. }) => {
			operation_stack.first().copied()
		}
		_ => None,
	}
}

/// Assemble this block's synthetic ethereum txs from `frame_system::Events`.
/// Backs the `SyntheticEthLogsApi`; uses the same `assemble_synth_txs` the
/// on-chain flusher uses, so output is byte-identical (A/B parity).
pub fn synthetic_transactions() -> Vec<(Transaction, TransactionStatus, Receipt)> {
	let records: Vec<_> = frame_system::Pallet::<Runtime>::read_events_no_consensus().collect();

	// Extrinsics that executed an ethereum tx: their substrate-dispatched
	// transfers/swaps already emitted inline logs into the real eth tx, so we
	// must not re-synthesize them. Identify by the `Executed` event they emit.
	let mut evm_extrinsics: BTreeSet<u32> = BTreeSet::new();
	for rec in records.iter() {
		if matches!(
			rec.event,
			RuntimeEvent::Ethereum(pallet_ethereum::Event::Executed { .. })
		) {
			if let Phase::ApplyExtrinsic(i) = rec.phase {
				evm_extrinsics.insert(i);
			}
		}
	}

	let mut entries: Vec<(Bucket, H160, ethereum::Log)> = Vec::new();
	for rec in records.iter() {
		let bucket = match rec.phase {
			Phase::ApplyExtrinsic(i) => {
				if evm_extrinsics.contains(&i) {
					continue;
				}
				Bucket::Extrinsic(i)
			}
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

	let chain_id = <Runtime as pallet_evm::Config>::ChainId::get();
	let parent_hash = frame_system::Pallet::<Runtime>::parent_hash();
	let block_number =
		UniqueSaturatedInto::<u64>::unique_saturated_into(frame_system::Pallet::<Runtime>::block_number());
	// real eth txs of this block are finalized into CurrentTransactionStatuses;
	// synth txs continue after them — matches the flusher's `Pending::count()`.
	let base_tx_index = pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
		.map(|s| s.len() as u32)
		.unwrap_or(0);

	assemble_synth_txs(entries, chain_id, parent_hash.as_ref(), block_number, base_tx_index)
}
