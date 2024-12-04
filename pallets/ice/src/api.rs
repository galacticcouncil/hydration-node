#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

use crate::types::{Intent, IntentId, ResolvedIntent};
use alloc::vec::Vec;
use codec::Decode;

sp_api::decl_runtime_apis! {
	#[api_version(1)]
	pub trait ICEApi<A, AssetId>
	where Vec<(IntentId, Intent<A, AssetId>)>: Decode,
	Vec<ResolvedIntent>: Decode
	{
		fn intents(header: &Block::Header) -> Vec<(IntentId, Intent<A, AssetId>)>;
		fn submit_solution(
			header: &Block::Header,
			solution: Vec<ResolvedIntent>,
		) -> Result<(), sp_runtime::DispatchError>;
	}
}
