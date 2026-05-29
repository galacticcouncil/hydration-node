// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! translates `Swapped3` events to uniswap-v2 `Swap` logs (every defi aggregator
//! indexes by that signature). v1: bilateral `ExactIn`/`ExactOut` only.

use crate::evm::precompiles::erc20_mapping::is_asset_address;
use crate::Runtime;
use hydradx_traits::evm::InspectEvmAccounts;
use pallet_broadcast::types::{Asset, ExecutionType, Fee, Filler, TradeOperation};
use pallet_broadcast::OnTrade;
use pallet_synthetic_logs::{build_uniswap_v2_swap_log, Pallet as SyntheticLogs};
use primitive_types::{H160, U256};
use primitives::AccountId;

pub struct EmitUniswapV2SwapLog;

fn evm_address_of(account: &AccountId) -> H160 {
	pallet_evm_accounts::Pallet::<Runtime>::evm_address(account)
}

// pool's evm address = h160-derivation of the filler's substrate accountid.
fn derive_pool_address(filler: &AccountId) -> H160 {
	evm_address_of(filler)
}

/// Translate a trade into its uniswap-v2 `Swap` log (emitter = pool's evm
/// address), or `None` for trades outside v1 scope. Shared by the `OnTrade`
/// hook (below) and the event-reader (`event_logs`), so both agree byte-for-byte.
///
/// v1: only bilateral `ExactIn`/`ExactOut` (single input and output asset).
pub fn swap_log(
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

	let pool_address = derive_pool_address(filler);
	// would clash with HydraErc20Mapping; skip (probability ~2^-128).
	if is_asset_address(pool_address) {
		log::warn!(
			target: "runtime::synthetic-logs",
			"pool address collides with erc20 prefix; skipping swap log for filler {filler:?}",
		);
		return None;
	}

	let sender = evm_address_of(swapper);
	let recipient = sender;

	// token0 = lower asset id, so amounts map deterministically.
	let input_is_token0 = in_asset <= out_asset;
	let log = build_uniswap_v2_swap_log(
		pool_address,
		sender,
		recipient,
		input_is_token0,
		U256::from(in_amount),
		U256::from(out_amount),
	);
	Some((pool_address, log))
}

impl OnTrade<AccountId> for EmitUniswapV2SwapLog {
	fn on_trade(
		swapper: &AccountId,
		filler: &AccountId,
		_filler_type: &Filler,
		operation: &TradeOperation,
		inputs: &[Asset],
		outputs: &[Asset],
		_fees: &[Fee<AccountId>],
		_operation_stack: &[ExecutionType],
	) {
		if let Some((pool_address, log)) = swap_log(swapper, filler, operation, inputs, outputs) {
			if !crate::evm::runner::append_to_current_evm_frame(log.clone()) {
				SyntheticLogs::<Runtime>::push(pool_address, log);
			}
		}
	}
}
