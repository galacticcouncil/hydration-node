use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::traits::ConstU32;
use sp_runtime::BoundedVec;

pub type Balance = u128;
pub type OrderId = u32;

#[derive(Encode, Decode, Debug, Eq, PartialEq, Clone, TypeInfo, MaxEncodedLen)]
pub struct Order<AssetId, BlockNumber> {
  pub asset_sell: AssetId,
  pub asset_buy: AssetId,
  pub amount_sell: Balance,
  pub amount_buy: Balance,
  pub expires: Option<BlockNumber>,
}
