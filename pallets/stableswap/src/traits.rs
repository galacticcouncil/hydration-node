use crate::PegSource;
use hydradx_traits::RawEntry;

pub trait PegOracle<AssetId, Balance, BlockNumber> {
	type Error;
	fn get(peg_asset: AssetId, source: PegSource<AssetId>) -> Result<RawEntry<Balance, BlockNumber>, Self::Error>;
}
