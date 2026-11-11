//! Identify XCM-transfer extrinsics. An `xtokens` / `pallet_xcm` / `orml_xcm` call by a signer moves
//! that account's OWN funds across chains — it is not a trade with a pool, so its net is neither
//! profit nor loss. The monitor marks these exts `is_xcm` so the per-account detectors skip them
//! (a pool such a transfer happens to touch is still covered by the beneficiary-agnostic pool-leak
//! detector).

use crate::provider::RemoteApi;
use codec::Decode;
use hydradx_runtime::{HydraUncheckedExtrinsic, RuntimeCall};
use sp_core::H256;
use std::collections::BTreeSet;

/// The indices of the block's extrinsics whose top-level call is an XCM transfer.
pub fn xcm_ext_indices(_api: &RemoteApi, _at: H256, extrinsics: &[Vec<u8>]) -> BTreeSet<u32> {
	extrinsics
		.iter()
		.enumerate()
		.filter(|(_, bytes)| is_xcm_transfer(bytes))
		.map(|(i, _)| i as u32)
		.collect()
}

/// Whether a single extrinsic's top-level call is an XCM transfer.
pub fn is_xcm_transfer(bytes: &[u8]) -> bool {
	let Ok(ext) = HydraUncheckedExtrinsic::decode(&mut &bytes[..]) else {
		return false;
	};
	matches!(
		ext.0.function,
		RuntimeCall::XTokens(_) | RuntimeCall::PolkadotXcm(_) | RuntimeCall::OrmlXcm(_)
	)
}
