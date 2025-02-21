use crate::types::{AssetId, Balance};
use sp_runtime::DispatchError;
use sp_std::vec::Vec;

pub trait Trader<AccountId> {
	type Outcome;
	/// Trade given assets
	///
	/// # Arguments
	/// `account` - the account that will trade
	/// `assets` - the assets to trade with their amounts (in , out)
	fn trade(account: AccountId, assets: Vec<(AssetId, (Balance, Balance))>) -> Result<Self::Outcome, DispatchError>;
}

impl<AccountId> Trader<AccountId> for () {
	type Outcome = ();

	fn trade(_account: AccountId, _assets: Vec<(AssetId, (Balance, Balance))>) -> Result<Self::Outcome, DispatchError> {
		Ok(())
	}
}
