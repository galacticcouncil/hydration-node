// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Node-side synthetic-logs indexing: surface substrate token/trade activity as
//! EVM logs over eth json-rpc, entirely off-chain.
//!
//! - [`storage_override`]: appends synthetic txs/statuses/receipts to Frontier's
//!   reads (header left canonical).
//! - [`eth_filter`]: custom `eth_getLogs` that surfaces synth logs without
//!   corrupting canonical block hashes.
//! - [`mapping_sync`]: vendored mapping-sync worker that also indexes the
//!   synthetic tx hashes so `eth_getTransactionByHash`/`*_receipt` resolve.

pub mod eth_filter;
pub mod mapping_sync;
pub mod storage_override;
