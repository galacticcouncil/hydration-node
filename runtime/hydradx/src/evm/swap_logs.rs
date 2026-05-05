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
use pallet_synthetic_logs::{encode_uint256_quad, h160_to_h256, Pallet as SyntheticLogs, SWAP_TOPIC};
use primitive_types::{H160, U256};
use primitives::AccountId;
use sp_std::vec;

pub struct EmitUniswapV2SwapLog;

fn evm_address_of(account: &AccountId) -> H160 {
	pallet_evm_accounts::Pallet::<Runtime>::evm_address(account)
}

// pool's evm address = h160-derivation of the filler's substrate accountid.
fn derive_pool_address(filler: &AccountId) -> H160 {
	evm_address_of(filler)
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
		if !matches!(operation, TradeOperation::ExactIn | TradeOperation::ExactOut) {
			return;
		}
		if inputs.len() != 1 || outputs.len() != 1 {
			return;
		}

		let in_asset = inputs[0].asset;
		let in_amount = inputs[0].amount;
		let out_asset = outputs[0].asset;
		let out_amount = outputs[0].amount;

		if in_amount == 0 && out_amount == 0 {
			return;
		}

		// token0 < token1 by id so amounts map deterministically.
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

		let pool_address = derive_pool_address(filler);
		// would clash with HydraErc20Mapping; skip (probability ~2^-128).
		if is_asset_address(pool_address) {
			log::warn!(
				target: "runtime::synthetic-logs",
				"pool address collides with erc20 prefix; skipping swap log for filler {:?}",
				filler,
			);
			return;
		}

		let sender = evm_address_of(swapper);
		let recipient = sender;

		let log = ethereum::Log {
			address: pool_address,
			topics: vec![SWAP_TOPIC, h160_to_h256(sender), h160_to_h256(recipient)],
			data: encode_uint256_quad(a0_in, a1_in, a0_out, a1_out),
		};

		if !crate::evm::runner::append_to_current_evm_frame(log.clone()) {
			SyntheticLogs::<Runtime>::push(pool_address, log);
		}
	}
}
