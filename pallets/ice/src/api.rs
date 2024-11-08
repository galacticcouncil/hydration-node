#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

use crate::types::{Intent, IntentId};
use alloc::vec::Vec;
use codec::Decode;

sp_api::decl_runtime_apis! {
	#[api_version(1)]
	pub trait ICEApi<A, AssetId>
	where Vec<(IntentId, Intent<A, AssetId>)>: Decode
	{
		fn intents(header: &Block::Header) -> Vec<(IntentId, Intent<A, AssetId>)>;
	}
}
