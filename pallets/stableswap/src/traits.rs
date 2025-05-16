use crate::PegSource;
use hydradx_traits::RawEntry;

pub trait PegRawOracle<AssetId, Balance, BlockNumber> {
	type Error;
	fn get_raw_entry(
		peg_asset: AssetId,
		source: PegSource<AssetId>,
	) -> Result<RawEntry<Balance, BlockNumber>, Self::Error>;
}
