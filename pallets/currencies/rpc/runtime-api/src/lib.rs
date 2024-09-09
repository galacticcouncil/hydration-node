#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime::codec::Codec;

sp_api::decl_runtime_apis! {
	pub trait CurrenciesApi<AssetId, AccountId, Balance> where
		AssetId: Codec,
		AccountId: Codec,
		Balance: Codec,
	{
		fn free_balance(asset_id: AssetId, who: AccountId) -> Balance;
	}
}
