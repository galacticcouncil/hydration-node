#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::all)]

use sp_runtime::{
	codec::Codec,
	traits::{MaybeDisplay, MaybeFromStr},
};

sp_api::decl_runtime_apis! {
	#[api_version(1)]
	pub trait StakingApi<AccountId> where
		AccountId: Codec + MaybeDisplay + MaybeFromStr ,
	{
		fn retrieve_account_points(
			who: AccountId,
		) -> u32;
	}
}
