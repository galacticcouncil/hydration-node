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
use hydradx_traits::evm::InspectEvmAccounts;
use primitive_types::H160;
use primitives::constants::chain::CORE_ASSET_ID;
use primitives::AccountId;
use sp_std::vec::Vec;

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
