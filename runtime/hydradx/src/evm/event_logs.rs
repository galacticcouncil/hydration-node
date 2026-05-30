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
//! Scope: every balance movement (orml-tokens + native HDX via pallet-balances)
//! — transfer, mint, burn/slash/dust, reserve/unreserve, repatriate, and
//! lock/freeze — mapped to erc20 `Transfer` so a holder's aggregated transfers
//! reconstruct its transferable balance (reserved and frozen amounts move to
//! distinct per-owner sentinels). Plus `Swapped3` → uniswap-v2 `Swap`, and
//! `pallet_evm::Event::Log` from internal `Executor::call` paths (deduped
//! against real eth txs). This is the off-chain replica of what the removed
//! on-chain synthetic-logs hooks emitted.

use crate::{Runtime, RuntimeEvent};
use frame_support::traits::Get;
use frame_system::{EventRecord, Phase};
use pallet_broadcast::types::{Asset, ExecutionType, TradeOperation};
use pallet_ethereum::{Receipt, Transaction, TransactionStatus};
use pallet_synthetic_logs::{
	account_to_evm_address, assemble_synth_txs, asset_evm_address, build_erc20_transfer_log, build_uniswap_v2_swap_log,
	frozen_address_of, reserved_address_of, Bucket, HookPhase,
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

const ZERO: H160 = H160([0u8; 20]);

fn evm_addr(account: &AccountId) -> H160 {
	account_to_evm_address(account.as_ref())
}

/// reserve repatriation → `Transfer(reserved_sentinel(from), to_or_its_reserved_sentinel)`.
fn repatriate(
	asset: u32,
	from_owner: H160,
	to_owner: H160,
	amount: u128,
	to_reserved: bool,
) -> Vec<(H160, ethereum::Log)> {
	let to = if to_reserved {
		reserved_address_of(to_owner)
	} else {
		to_owner
	};
	transfer(asset, reserved_address_of(from_owner), to, amount)
}

fn to_reserved(status: &orml_traits::BalanceStatus) -> bool {
	matches!(status, orml_traits::BalanceStatus::Reserved)
}

// Pure mirror of `erc20_mapping::is_asset_address`: prefix `0x..01` (15 zero
// bytes then `1`), the range `asset_evm_address` produces.
fn is_asset_address(addr: H160) -> bool {
	addr.0[..15] == [0u8; 15] && addr.0[15] == 1
}

/// One erc20 `Transfer(from, to, amount)` on `asset`'s evm address, or empty for
/// a zero amount. The single primitive every balance movement below maps onto:
/// mint = `from` zero, burn = `to` zero, reserve/lock = `to` the owner's
/// reserved/frozen sentinel. Aggregating these per (asset, holder) reconstructs
/// the holder's transferable balance.
fn transfer(asset: u32, from: H160, to: H160, amount: u128) -> Vec<(H160, ethereum::Log)> {
	if amount == 0 {
		return Vec::new();
	}
	let addr = asset_evm_address(asset);
	sp_std::vec![(addr, build_erc20_transfer_log(addr, from, to, U256::from(amount)))]
}

/// Pure: evm logs an indexer should see for one runtime event.
///
/// Covers every balance movement the on-chain synthetic-logs hooks did, now
/// driven off-chain from events: transfer, deposit/mint (`0x0 → who`),
/// withdraw/slash/burn/dust (`who → 0x0`), reserve/unreserve (`who ↔
/// reserved_address_of(who)`), reserve repatriation, plus lock/freeze (`who ↔
/// frozen_address_of(who)`, by frozen delta) so a holder's aggregated transfers
/// equal its transferable balance.
pub fn logs_from_event(event: &RuntimeEvent) -> Vec<(H160, ethereum::Log)> {
	use orml_tokens::Event as Tokens;
	use pallet_balances::Event as Balances;
	match event {
		// ---- orml-tokens (non-native assets) ----
		RuntimeEvent::Tokens(Tokens::Transfer {
			currency_id,
			from,
			to,
			amount,
		}) => transfer(*currency_id, evm_addr(from), evm_addr(to), *amount),
		RuntimeEvent::Tokens(Tokens::Deposited {
			currency_id,
			who,
			amount,
		}) => transfer(*currency_id, ZERO, evm_addr(who), *amount),
		RuntimeEvent::Tokens(Tokens::Withdrawn {
			currency_id,
			who,
			amount,
		})
		| RuntimeEvent::Tokens(Tokens::DustLost {
			currency_id,
			who,
			amount,
		}) => transfer(*currency_id, evm_addr(who), ZERO, *amount),
		RuntimeEvent::Tokens(Tokens::Slashed {
			currency_id,
			who,
			free_amount,
			reserved_amount,
		}) => {
			let owner = evm_addr(who);
			let mut logs = transfer(*currency_id, owner, ZERO, *free_amount);
			logs.extend(transfer(
				*currency_id,
				reserved_address_of(owner),
				ZERO,
				*reserved_amount,
			));
			logs
		}
		RuntimeEvent::Tokens(Tokens::Reserved {
			currency_id,
			who,
			amount,
		}) => {
			let owner = evm_addr(who);
			transfer(*currency_id, owner, reserved_address_of(owner), *amount)
		}
		RuntimeEvent::Tokens(Tokens::Unreserved {
			currency_id,
			who,
			amount,
		}) => {
			let owner = evm_addr(who);
			transfer(*currency_id, reserved_address_of(owner), owner, *amount)
		}
		RuntimeEvent::Tokens(Tokens::ReserveRepatriated {
			currency_id,
			from,
			to,
			amount,
			status,
		}) => repatriate(*currency_id, evm_addr(from), evm_addr(to), *amount, to_reserved(status)),
		RuntimeEvent::Tokens(Tokens::Locked {
			currency_id,
			who,
			amount,
		}) => {
			let owner = evm_addr(who);
			transfer(*currency_id, owner, frozen_address_of(owner), *amount)
		}
		RuntimeEvent::Tokens(Tokens::Unlocked {
			currency_id,
			who,
			amount,
		}) => {
			let owner = evm_addr(who);
			transfer(*currency_id, frozen_address_of(owner), owner, *amount)
		}

		// ---- native HDX (pallet-balances) ----
		RuntimeEvent::Balances(Balances::Transfer { from, to, amount }) => {
			transfer(CORE_ASSET_ID, evm_addr(from), evm_addr(to), *amount)
		}
		// Mint: `Deposit` (Currency api) and `Minted` (fungible api) are emitted by
		// disjoint code paths — a given increase emits exactly one, so mapping both
		// captures every increase once.
		RuntimeEvent::Balances(Balances::Deposit { who, amount })
		| RuntimeEvent::Balances(Balances::Minted { who, amount }) => transfer(CORE_ASSET_ID, ZERO, evm_addr(who), *amount),
		// Burn: likewise `Withdraw`/`Burned`/`Slashed`/`DustLost` are disjoint.
		RuntimeEvent::Balances(Balances::Withdraw { who, amount })
		| RuntimeEvent::Balances(Balances::Burned { who, amount })
		| RuntimeEvent::Balances(Balances::Slashed { who, amount })
		| RuntimeEvent::Balances(Balances::DustLost { account: who, amount }) => {
			transfer(CORE_ASSET_ID, evm_addr(who), ZERO, *amount)
		}
		RuntimeEvent::Balances(Balances::Reserved { who, amount }) => {
			let owner = evm_addr(who);
			transfer(CORE_ASSET_ID, owner, reserved_address_of(owner), *amount)
		}
		RuntimeEvent::Balances(Balances::Unreserved { who, amount }) => {
			let owner = evm_addr(who);
			transfer(CORE_ASSET_ID, reserved_address_of(owner), owner, *amount)
		}
		RuntimeEvent::Balances(Balances::ReserveRepatriated {
			from,
			to,
			amount,
			destination_status,
		}) => repatriate(
			CORE_ASSET_ID,
			evm_addr(from),
			evm_addr(to),
			*amount,
			matches!(
				destination_status,
				frame_support::traits::tokens::BalanceStatus::Reserved
			),
		),
		// Lock + freeze both adjust `frozen`; each event carries the frozen delta.
		RuntimeEvent::Balances(Balances::Locked { who, amount })
		| RuntimeEvent::Balances(Balances::Frozen { who, amount }) => {
			let owner = evm_addr(who);
			transfer(CORE_ASSET_ID, owner, frozen_address_of(owner), *amount)
		}
		RuntimeEvent::Balances(Balances::Unlocked { who, amount })
		| RuntimeEvent::Balances(Balances::Thawed { who, amount }) => {
			let owner = evm_addr(who);
			transfer(CORE_ASSET_ID, frozen_address_of(owner), owner, *amount)
		}

		// ---- swaps ----
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

#[cfg(test)]
mod tests {
	use super::*;
	use orml_tokens::Event as Tokens;
	use pallet_balances::Event as Balances;
	use pretty_assertions::assert_eq;

	const HDX: u32 = CORE_ASSET_ID;
	const DAI: u32 = 2;

	fn acc(n: u8) -> AccountId {
		AccountId::from([n; 32])
	}
	fn owner_of(n: u8) -> H160 {
		H160([n; 20])
	}
	fn from_of(log: &ethereum::Log) -> H160 {
		H160::from_slice(&log.topics[1].0[12..])
	}
	fn to_of(log: &ethereum::Log) -> H160 {
		H160::from_slice(&log.topics[2].0[12..])
	}
	fn amount_of(log: &ethereum::Log) -> u128 {
		U256::from_big_endian(&log.data).low_u128()
	}
	fn one(event: RuntimeEvent) -> (H160, ethereum::Log) {
		let logs = logs_from_event(&event);
		assert_eq!(logs.len(), 1, "expected exactly one log");
		logs.into_iter().next().unwrap()
	}

	#[test]
	fn token_transfer_should_map_to_erc20_transfer_on_asset_address() {
		let (emitter, log) = one(RuntimeEvent::Tokens(Tokens::Transfer {
			currency_id: DAI,
			from: acc(1),
			to: acc(2),
			amount: 500,
		}));
		assert_eq!(emitter, asset_evm_address(DAI));
		assert_eq!(from_of(&log), owner_of(1));
		assert_eq!(to_of(&log), owner_of(2));
		assert_eq!(amount_of(&log), 500);
	}

	#[test]
	fn native_transfer_should_map_to_core_asset_address() {
		let (emitter, _) = one(RuntimeEvent::Balances(Balances::Transfer {
			from: acc(1),
			to: acc(2),
			amount: 7,
		}));
		assert_eq!(emitter, asset_evm_address(HDX));
	}

	#[test]
	fn deposit_and_mint_should_map_to_transfer_from_zero() {
		for ev in [
			RuntimeEvent::Tokens(Tokens::Deposited {
				currency_id: DAI,
				who: acc(1),
				amount: 9,
			}),
			RuntimeEvent::Balances(Balances::Deposit { who: acc(1), amount: 9 }),
			RuntimeEvent::Balances(Balances::Minted { who: acc(1), amount: 9 }),
		] {
			let (_, log) = one(ev);
			assert_eq!(from_of(&log), ZERO);
			assert_eq!(to_of(&log), owner_of(1));
		}
	}

	#[test]
	fn withdraw_burn_slash_dust_should_map_to_transfer_to_zero() {
		for ev in [
			RuntimeEvent::Tokens(Tokens::Withdrawn {
				currency_id: DAI,
				who: acc(1),
				amount: 3,
			}),
			RuntimeEvent::Balances(Balances::Withdraw { who: acc(1), amount: 3 }),
			RuntimeEvent::Balances(Balances::Burned { who: acc(1), amount: 3 }),
			RuntimeEvent::Balances(Balances::Slashed { who: acc(1), amount: 3 }),
			RuntimeEvent::Balances(Balances::DustLost {
				account: acc(1),
				amount: 3,
			}),
		] {
			let (_, log) = one(ev);
			assert_eq!(from_of(&log), owner_of(1));
			assert_eq!(to_of(&log), ZERO);
		}
	}

	#[test]
	fn reserve_should_move_to_reserved_sentinel_and_unreserve_back() {
		let owner = owner_of(1);
		let (_, r) = one(RuntimeEvent::Balances(Balances::Reserved { who: acc(1), amount: 4 }));
		assert_eq!(from_of(&r), owner);
		assert_eq!(to_of(&r), reserved_address_of(owner));
		let (_, u) = one(RuntimeEvent::Balances(Balances::Unreserved { who: acc(1), amount: 4 }));
		assert_eq!(from_of(&u), reserved_address_of(owner));
		assert_eq!(to_of(&u), owner);
	}

	#[test]
	fn lock_should_use_frozen_sentinel_distinct_from_reserved() {
		let owner = owner_of(1);
		assert_ne!(frozen_address_of(owner), reserved_address_of(owner));
		let (_, l) = one(RuntimeEvent::Balances(Balances::Frozen { who: acc(1), amount: 6 }));
		assert_eq!(from_of(&l), owner);
		assert_eq!(to_of(&l), frozen_address_of(owner));
		let (_, t) = one(RuntimeEvent::Balances(Balances::Thawed { who: acc(1), amount: 6 }));
		assert_eq!(from_of(&t), frozen_address_of(owner));
		assert_eq!(to_of(&t), owner);
	}

	#[test]
	fn token_slash_should_split_free_and_reserved() {
		let owner = owner_of(1);
		let logs = logs_from_event(&RuntimeEvent::Tokens(Tokens::Slashed {
			currency_id: DAI,
			who: acc(1),
			free_amount: 10,
			reserved_amount: 5,
		}));
		assert_eq!(logs.len(), 2);
		assert_eq!(from_of(&logs[0].1), owner);
		assert_eq!(amount_of(&logs[0].1), 10);
		assert_eq!(from_of(&logs[1].1), reserved_address_of(owner));
		assert_eq!(amount_of(&logs[1].1), 5);
		assert!(logs.iter().all(|(_, l)| to_of(l) == ZERO));
	}

	#[test]
	fn zero_amount_should_emit_no_log() {
		assert!(logs_from_event(&RuntimeEvent::Balances(Balances::Transfer {
			from: acc(1),
			to: acc(2),
			amount: 0,
		}))
		.is_empty());
	}

	// the headline invariant: aggregating (incoming − outgoing) erc20 transfer
	// amounts for an owner reconstructs its *transferable* balance.
	#[test]
	fn aggregated_transfers_should_reconstruct_transferable_balance() {
		let owner = owner_of(1);
		// free 100, then reserve 20 (free→reserved), then freeze 30 (lien on free).
		// transferable = free(80) − frozen(30) = 50.
		let events = [
			RuntimeEvent::Balances(Balances::Minted {
				who: acc(1),
				amount: 100,
			}),
			RuntimeEvent::Balances(Balances::Reserved {
				who: acc(1),
				amount: 20,
			}),
			RuntimeEvent::Balances(Balances::Frozen {
				who: acc(1),
				amount: 30,
			}),
		];
		let net = |target: H160| -> i128 {
			let mut bal = 0i128;
			for ev in events.iter() {
				for (_, log) in logs_from_event(ev) {
					if to_of(&log) == target {
						bal += amount_of(&log) as i128;
					}
					if from_of(&log) == target {
						bal -= amount_of(&log) as i128;
					}
				}
			}
			bal
		};
		assert_eq!(net(owner), 50, "owner aggregate must equal transferable balance");
		assert_eq!(net(reserved_address_of(owner)), 20, "reserved sentinel holds reserved");
		assert_eq!(net(frozen_address_of(owner)), 30, "frozen sentinel holds frozen");
	}
}
