use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::traits::ConstU32;
use sp_runtime::BoundedVec;

pub type Balance = u128;
pub type OrderId = u32;

#[derive(Encode, Decode, Debug, Eq, PartialEq, Clone, TypeInfo, MaxEncodedLen)]
pub struct Order<AccountId, AssetId> {
  pub who: AccountId,
  pub asset_buy: AssetId,
  pub asset_sell: AssetId,
  pub amount_buy: Balance,
  pub amount_sell: Balance,
  pub partially_fillable: bool,
}
