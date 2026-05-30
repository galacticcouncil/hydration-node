// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! ERC-20 `Transfer` log construction for the node-indexing variant. The
//! event-reader (`event_logs`) calls `transfer_log` to turn a token movement
//! into its evm log; there are no on-chain mutation hooks — substrate-origin
//! transfers are surfaced off-chain by `SyntheticEthLogsApi`.

use crate::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use hydradx_traits::evm::Erc20Mapping;
use pallet_synthetic_logs::build_erc20_transfer_log;
use primitive_types::{H160, U256};
use primitives::{AssetId, Balance};

/// Resolve a token movement to its ERC-20 `Transfer` log; emitter is the
/// asset's evm address.
pub fn transfer_log(asset: AssetId, from: H160, to: H160, amount: Balance) -> (H160, ethereum::Log) {
	let address = HydraErc20Mapping::asset_address(asset);
	(address, build_erc20_transfer_log(address, from, to, U256::from(amount)))
}
