// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Node-side synthetic-logs indexing: surface substrate token/trade activity as
//! EVM logs over eth json-rpc, entirely off-chain.
//!
//! - [`metadata_events`]: decodes `System::Events` against the *chain's* own runtime
//!   metadata, so a node built on a different polkadot-sdk reads events by name rather
//!   than by variant index — and measures records it cannot represent instead of
//!   losing their neighbours.
//! - [`compat_events`]: picks a reader for a block and falls back to hand-written
//!   layouts and a salvage scan when metadata is unavailable.
//! - [`storage_override`]: appends synthetic txs/statuses/receipts to Frontier's
//!   reads (header left canonical).
//! - [`eth_filter`]: custom `eth_getLogs` that surfaces synth logs without
//!   corrupting canonical block hashes.
//! - [`mapping_sync`]: vendored mapping-sync worker that also indexes the
//!   synthetic tx hashes so `eth_getTransactionByHash`/`*_receipt` resolve.

pub mod compat_events;
pub mod eth_filter;
pub mod mapping_sync;
pub mod metadata_events;
pub mod storage_override;
