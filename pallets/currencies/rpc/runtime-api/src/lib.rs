#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Codec, Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;
use sp_std::vec::Vec;

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct AccountData<Balance> {
	/// Non-reserved part of the balance. There may still be restrictions on
	/// this, but it is the total pool what may in principle be transferred,
	/// reserved.
	///
	/// This is the only balance that matters in terms of most operations on
	/// tokens.
	pub free: Balance,
	/// Balance which is reserved and may not be used at all.
	///
	/// This can still get slashed, but gets slashed last of all.
	///
	/// This balance is a 'reserve' balance that other subsystems use in
	/// order to set aside tokens that are still 'owned' by the account
	/// holder, but which are suspendable.
	pub reserved: Balance,
	/// The amount that `free` may not drop below when withdrawing.
	pub frozen: Balance,
}

sp_api::decl_runtime_apis! {
	pub trait CurrenciesApi<AssetId, AccountId, Balance> where
		AssetId: Codec,
		AccountId: Codec,
		Balance: Codec,
	{
		fn account(asset_id: AssetId, who: AccountId) -> AccountData<Balance>;
		fn accounts(who: AccountId) -> Vec<(AssetId, AccountData<Balance>)>;
		fn free_balance(asset_id: AssetId, who: AccountId) -> Balance;
	}
}
