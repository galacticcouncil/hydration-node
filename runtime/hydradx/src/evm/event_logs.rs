// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Pure event → synthetic-eth-tx translation for the node-indexing variant.
//!
//! `synthetic_txs_from_records` turns a block's `EventRecord`s into
//! `(Transaction, TransactionStatus, Receipt)` triples with no state reads, so
//! the node's client-side indexer can call it for any runtime version. Balance
//! movements map to erc20 `Transfer` (reserved/frozen go to per-owner sentinels
//! so aggregated transfers equal an account's transferable balance), plus
//! `Swapped3` → uniswap-v2 `Swap` and internal `pallet_evm::Log` (deduped vs
//! real eth txs).

use super::synthetic_logs::{
	account_to_evm_address, assemble_synth_txs, asset_evm_address, build_erc20_transfer_log, build_uniswap_v2_swap_log,
	frozen_address_of, reserved_address_of, Bucket, HookPhase,
};
use crate::RuntimeEvent;
use frame_system::{EventRecord, Phase};
use pallet_broadcast::types::{Asset, ExecutionType, TradeOperation};
use pallet_ethereum::{Receipt, Transaction, TransactionStatus};
use primitive_types::{H160, U256};
use primitives::constants::chain::CORE_ASSET_ID;
use primitives::AccountId;
use sp_core::H256;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

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

/// One erc20 `Transfer` on `asset`'s evm address (empty for zero). Mint = `from`
/// zero, burn = `to` zero, reserve/lock = `to` the owner's sentinel.
fn transfer(asset: u32, from: H160, to: H160, amount: u128) -> Vec<(H160, ethereum::Log)> {
	if amount == 0 {
		return Vec::new();
	}
	let addr = asset_evm_address(asset);
	sp_std::vec![(addr, build_erc20_transfer_log(addr, from, to, U256::from(amount)))]
}

/// Pure: the evm logs an indexer should see for one runtime event.
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
		// Burn: `Withdraw`/`Burned`/`DustLost` are disjoint reductions of free balance.
		RuntimeEvent::Balances(Balances::Withdraw { who, amount })
		| RuntimeEvent::Balances(Balances::Burned { who, amount })
		| RuntimeEvent::Balances(Balances::DustLost { account: who, amount }) => {
			transfer(CORE_ASSET_ID, evm_addr(who), ZERO, *amount)
		}
		// Native slashing here only ever hits reserved balance (governance bonds via
		// democracy/elections); the event can't distinguish, so burn the reserved sentinel.
		RuntimeEvent::Balances(Balances::Slashed { who, amount }) => {
			transfer(CORE_ASSET_ID, reserved_address_of(evm_addr(who)), ZERO, *amount)
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

type LogKey = (H160, Vec<H256>, Vec<u8>);

fn log_key(address: H160, topics: &[H256], data: &[u8]) -> LogKey {
	(address, topics.to_vec(), data.to_vec())
}

/// PURE: assemble a block's synthetic txs from its event records. Shared by the
/// runtime API and the node's client-side indexer. `real_statuses` are the
/// block's real eth-tx statuses — used both to index synth txs after them
/// (`base_tx_index`) and to dedup `EVM::Log` events that already appear in a
/// real tx's receipt.
pub fn synthetic_txs_from_records(
	records: &[EventRecord<RuntimeEvent, H256>],
	chain_id: u64,
	parent_hash: &[u8],
	block_number: u64,
	real_statuses: &[TransactionStatus],
) -> Vec<(Transaction, TransactionStatus, Receipt)> {
	let base_tx_index = real_statuses.len() as u32;

	// An EVM log is also recorded in the receipt of the real eth tx in its extrinsic,
	// so drop EVM::Log events matching that receipt (one-for-one); extra logs from a
	// separate internal Executor::call survive. Real statuses follow Executed order.
	let mut real_logs_by_ext: BTreeMap<u32, BTreeMap<LogKey, u32>> = BTreeMap::new();
	let mut nth_eth_tx = 0usize;
	for rec in records.iter() {
		if matches!(
			rec.event,
			RuntimeEvent::Ethereum(pallet_ethereum::Event::Executed { .. })
		) {
			if let Phase::ApplyExtrinsic(i) = rec.phase {
				if let Some(status) = real_statuses.get(nth_eth_tx) {
					let ms = real_logs_by_ext.entry(i).or_default();
					for log in status.logs.iter() {
						*ms.entry(log_key(log.address, &log.topics, &log.data)).or_insert(0) += 1;
					}
				}
			}
			nth_eth_tx += 1;
		}
	}

	let mut entries: Vec<(Bucket, H160, ethereum::Log)> = Vec::new();
	for rec in records.iter() {
		if let RuntimeEvent::EVM(pallet_evm::Event::Log { log }) = &rec.event {
			if let Phase::ApplyExtrinsic(i) = rec.phase {
				if let Some(ms) = real_logs_by_ext.get_mut(&i) {
					if let Some(count) = ms.get_mut(&log_key(log.address, &log.topics, &log.data)) {
						if *count > 0 {
							*count -= 1;
							continue;
						}
					}
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
	fn withdraw_burn_dust_should_map_to_transfer_to_zero() {
		for ev in [
			RuntimeEvent::Tokens(Tokens::Withdrawn {
				currency_id: DAI,
				who: acc(1),
				amount: 3,
			}),
			RuntimeEvent::Balances(Balances::Withdraw { who: acc(1), amount: 3 }),
			RuntimeEvent::Balances(Balances::Burned { who: acc(1), amount: 3 }),
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
	fn native_slash_should_burn_from_reserved_sentinel() {
		// native slashing only ever hits reserved balance in this runtime.
		let (_, log) = one(RuntimeEvent::Balances(Balances::Slashed { who: acc(1), amount: 3 }));
		assert_eq!(from_of(&log), reserved_address_of(owner_of(1)));
		assert_eq!(to_of(&log), ZERO);
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

	// gap-2: an `EVM::Log` already in the real eth tx's receipt is deduped, but an
	// extra log from a separate internal call in the same extrinsic survives.
	#[test]
	fn evm_log_dedup_should_drop_receipt_duplicates_but_keep_internal_logs() {
		// the event carries `fp_evm::Log`; the receipt carries `ethereum::Log`.
		let dup_addr = H160([9u8; 20]);
		let internal_addr = H160([8u8; 20]);
		let evm_log = |addr: H160, t: u8, d: u8| fp_evm::Log {
			address: addr,
			topics: sp_std::vec![H256([t; 32])],
			data: sp_std::vec![d],
		};
		let receipt_dup = ethereum::Log {
			address: dup_addr,
			topics: sp_std::vec![H256([1u8; 32])],
			data: sp_std::vec![1],
		};
		let rec = |event| EventRecord {
			phase: Phase::ApplyExtrinsic(0),
			event,
			topics: sp_std::vec![],
		};
		let records = sp_std::vec![
			rec(RuntimeEvent::Ethereum(pallet_ethereum::Event::Executed {
				from: H160::zero(),
				to: H160::zero(),
				transaction_hash: H256::zero(),
				exit_reason: fp_evm::ExitReason::Succeed(fp_evm::ExitSucceed::Returned),
				extra_data: sp_std::vec![],
			})),
			rec(RuntimeEvent::EVM(pallet_evm::Event::Log {
				log: evm_log(dup_addr, 1, 1),
			})),
			rec(RuntimeEvent::EVM(pallet_evm::Event::Log {
				log: evm_log(internal_addr, 2, 4),
			})),
		];
		let real = sp_std::vec![TransactionStatus {
			transaction_hash: H256::zero(),
			transaction_index: 0,
			from: H160::zero(),
			to: None,
			contract_address: None,
			logs: sp_std::vec![receipt_dup],
			logs_bloom: Default::default(),
		}];
		let out = synthetic_txs_from_records(&records, 1, &[0u8; 32], 1, &real);
		let synth_logs: Vec<ethereum::Log> = out.iter().flat_map(|(_, s, _)| s.logs.clone()).collect();
		assert!(
			synth_logs.iter().any(|l| l.address == internal_addr),
			"internal-call log must be synthesized"
		);
		assert!(
			!synth_logs.iter().any(|l| l.address == dup_addr),
			"receipt-duplicate log must be dropped"
		);
		// synth tx indices continue after the one real eth tx.
		assert_eq!(out[0].1.transaction_index, 1);
	}
}
