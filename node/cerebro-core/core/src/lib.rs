//! Detection core: decides whether per-account profit on Hydration is suspicious.
//!
//! Everything here is chain-transport-agnostic: input is the normalized per-block event
//! model ([`model`]) produced by a chain mapper, plus the reader traits ([`reader`]) for
//! the few state reads detection needs mid-flight. No RPC, no runtime types, no storage
//! encoding. See `docs/detection-model.md` for the rules and the false-positive history
//! that shaped them.

pub mod accounts;
pub mod confirm;
pub mod detect;
pub mod model;
pub mod pool_leak;
pub mod prices;
pub mod reader;
pub mod reduce;
pub mod value;

pub type AssetId = u32;
pub type Balance = u128;
/// 32-byte substrate account id (raw; SS58 rendering is a presentation concern).
pub type AccountId = [u8; 32];
/// 20-byte EVM address.
pub type EvmAddress = [u8; 20];
/// Block hash — opaque identity handed back to the reader traits.
pub type BlockHash = [u8; 32];

/// The native asset (HDX). Fees are paid in it; several rules treat it specially.
pub const NATIVE_ASSET: AssetId = 0;

/// Movements at or below this (raw units) are dust — ignored in the flow-graph view.
pub const DUST: i128 = 1_000;
