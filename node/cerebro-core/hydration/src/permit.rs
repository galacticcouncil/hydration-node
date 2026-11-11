//! Attribute EVM-permit trades to their real actor.
//!
//! A `MultiTransactionPayment::dispatch_permit` carries the actor's EVM address as its first arg
//! `from`; the fee-payer is a paymaster (or nobody), so `TransactionFeePaid.who` is the wrong actor.
//! We recover the true one from the call and map the EVM address to its substrate account.

use crate::provider::RemoteApi;
use crate::transport::Transport;
use cerebro_core::AccountId as CoreAccountId;
use codec::Decode;
use hydradx_runtime::{HydraUncheckedExtrinsic, Runtime, RuntimeCall};
use pallet_evm_accounts::AccountExtension;
use primitives::AccountId;
use sp_core::{H160, H256};
use std::collections::HashMap;

/// For each extrinsic index that is a `dispatch_permit`, the real actor's account (core's raw id).
pub fn permit_actors(api: &RemoteApi, at: H256, extrinsics: &[Vec<u8>]) -> HashMap<u32, CoreAccountId> {
	let mut out = HashMap::new();
	for (i, bytes) in extrinsics.iter().enumerate() {
		if let Some(from) = decode_permit_from(bytes) {
			out.insert(i as u32, account_id_of_evm(api, at, from).into());
		}
	}
	out
}

/// Debug: `pallet.call` index of every decodable extrinsic (permit = pallet 203 call 4).
pub fn dump_calls(extrinsics: &[Vec<u8>]) -> Vec<String> {
	use codec::Encode;
	extrinsics
		.iter()
		.enumerate()
		.map(|(i, b)| match HydraUncheckedExtrinsic::decode(&mut &b[..]) {
			Ok(ext) => {
				let enc = ext.0.function.encode();
				format!(
					"{i}:p{}.c{}",
					enc.first().copied().unwrap_or(0),
					enc.get(1).copied().unwrap_or(0)
				)
			}
			Err(e) => format!("{i}:<err {e}>"),
		})
		.collect()
}

/// Readable call of every extrinsic (`RuntimeCall` debug, truncated) — evidence for the AI reviewer.
/// An undecodable call means the runtime is ahead of the compiled types.
pub fn describe_calls(extrinsics: &[Vec<u8>]) -> Vec<String> {
	const MAX: usize = 1500;
	extrinsics
		.iter()
		.map(|b| match HydraUncheckedExtrinsic::decode(&mut &b[..]) {
			Ok(ext) => {
				let mut s = format!("{:?}", ext.0.function);
				if s.len() > MAX {
					let cut = (0..=MAX).rev().find(|&i| s.is_char_boundary(i)).unwrap_or(0);
					s.truncate(cut);
					s.push('…');
				}
				s
			}
			Err(e) => format!("<undecodable with compiled runtime types: {e}>"),
		})
		.collect()
}

/// The real actor of a single extrinsic if it is a `dispatch_permit`.
pub fn permit_actor<T: Transport>(api: &T, at: H256, extrinsic: &[u8]) -> Option<CoreAccountId> {
	decode_permit_from(extrinsic).map(|from| account_id_of_evm(api, at, from).into())
}

fn decode_permit_from(bytes: &[u8]) -> Option<H160> {
	let ext = HydraUncheckedExtrinsic::decode(&mut &bytes[..]).ok()?;
	match ext.0.function {
		RuntimeCall::MultiTransactionPayment(pallet_transaction_multi_payment::Call::dispatch_permit {
			from, ..
		}) => Some(from),
		_ => None,
	}
}

/// `pallet_evm_accounts::account_id(from)`: the bound account (`from ++ last-12-bytes`) if bound,
/// else the `ETH\0`-truncated mirror.
fn account_id_of_evm<T: Transport>(api: &T, at: H256, from: H160) -> AccountId {
	let key = AccountExtension::<Runtime>::hashed_key_for(from);
	if let Ok(Some(raw)) = api.storage(&key, at.0) {
		if let Ok(last12) = <[u8; 12]>::decode(&mut &raw[..]) {
			let mut data = [0u8; 32];
			data[..20].copy_from_slice(from.as_bytes());
			data[20..32].copy_from_slice(&last12);
			return AccountId::from(data);
		}
	}
	let mut data = [0u8; 32];
	data[..4].copy_from_slice(b"ETH\0");
	data[4..24].copy_from_slice(from.as_bytes());
	AccountId::from(data)
}
