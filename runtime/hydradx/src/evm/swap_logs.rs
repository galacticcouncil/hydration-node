// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Translates `pallet_broadcast::Event::Swapped3` trades into uniswap-v2-style
//! `Swap(address,uint256,uint256,uint256,uint256,address)` evm logs and
//! buffers them in `pallet_synthetic_logs`. Picked the uniswap v2 signature
//! because every defi aggregator (1inch, paraswap, defillama) and every dex
//! subgraph already indexes by it — emitting it from a stable per-pool
//! address gets us auto-discovery for free.
//!
//! v1 scope: bilateral 1-input / 1-output swaps (`ExactIn`/`ExactOut`).
//! Liquidity add/remove and multi-asset trades are skipped — they don't fit
//! the uniswap v2 mental model. Future extensions: `Mint`/`Burn` for
//! liquidity ops, custom `OTCFilled` for limit orders.

use crate::evm::precompiles::erc20_mapping::is_asset_address;
use crate::Runtime;
use hydradx_traits::evm::InspectEvmAccounts;
use pallet_broadcast::types::{Asset, ExecutionType, Fee, Filler, TradeOperation};
use pallet_broadcast::OnTrade;
use pallet_synthetic_logs::{encode_uint256_quad, h160_to_h256, Pallet as SyntheticLogs, SWAP_TOPIC};
use primitive_types::{H160, U256};
use primitives::AccountId;
use sp_std::vec;

pub struct EmitUniswapV2SwapLog;

fn evm_address_of(account: &AccountId) -> H160 {
	pallet_evm_accounts::Pallet::<Runtime>::evm_address(account)
}

/// Derive a stable evm address for a substrate-side pool (the trade `filler`).
///
/// Each pool has a unique substrate `AccountId` (omnipool's pallet account,
/// each stableswap pool's account, each xyk pool's share-token account). The
/// h160 derivation from that accountId is stable across blocks and 1:1 with
/// the pool — exactly what indexers need.
fn derive_pool_address(filler: &AccountId, _filler_type: &Filler) -> H160 {
	evm_address_of(filler)
}

impl OnTrade<AccountId> for EmitUniswapV2SwapLog {
	fn on_trade(
		swapper: &AccountId,
		filler: &AccountId,
		filler_type: &Filler,
		operation: &TradeOperation,
		inputs: &[Asset],
		outputs: &[Asset],
		_fees: &[Fee<AccountId>],
		_operation_stack: &[ExecutionType],
	) {
		// v1: only swap-shaped trades. Liquidity ops, OTC limit fills, and
		// multi-asset stableswap legs don't fit the uniswap v2 mental model.
		if !matches!(operation, TradeOperation::ExactIn | TradeOperation::ExactOut) {
			return;
		}
		if inputs.len() != 1 || outputs.len() != 1 {
			return;
		}

		// Today no swap precompile exists, so substrate-side trades never fire
		// while inside an evm execution frame. Defensive guard: if a future
		// swap precompile is added, it must emit the log inline itself; we
		// skip here to avoid double-emission.
		if crate::evm::runner::is_in_evm() {
			return;
		}

		let in_asset = inputs[0].asset;
		let in_amount = inputs[0].amount;
		let out_asset = outputs[0].asset;
		let out_amount = outputs[0].amount;

		if in_amount == 0 && out_amount == 0 {
			return;
		}

		// Sort assets by id so token0 < token1 deterministically. Indexers
		// assume this ordering when interpreting the four amount fields.
		let token0_id = in_asset.min(out_asset);
		let (a0_in, a1_in, a0_out, a1_out) = if in_asset == token0_id {
			(
				U256::from(in_amount),
				U256::zero(),
				U256::zero(),
				U256::from(out_amount),
			)
		} else {
			(
				U256::zero(),
				U256::from(in_amount),
				U256::from(out_amount),
				U256::zero(),
			)
		};

		let pool_address = derive_pool_address(filler, filler_type);
		// Defensive: pool address must not collide with the erc20 prefix range
		// used by `HydraErc20Mapping::asset_address`. If a pool's accountId
		// happened to derive to that range we'd emit logs from an asset
		// contract address by mistake. Skip silently if it does — this is
		// astronomically unlikely with any real pool accountId.
		if is_asset_address(pool_address) {
			log::warn!(
				target: "runtime::synthetic-logs",
				"pool address collides with erc20 prefix; skipping swap log for filler {:?}",
				filler,
			);
			return;
		}

		let sender = evm_address_of(swapper);
		// Hydration trades always credit the swapper. There's no separate
		// recipient field on `Swapped3` to encode here.
		let recipient = sender;

		let log = ethereum::Log {
			address: pool_address,
			topics: vec![SWAP_TOPIC, h160_to_h256(sender), h160_to_h256(recipient)],
			data: encode_uint256_quad(a0_in, a1_in, a0_out, a1_out),
		};

		SyntheticLogs::<Runtime>::push(pool_address, log);
	}
}
