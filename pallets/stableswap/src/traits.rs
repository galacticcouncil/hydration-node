use codec::{Decode, Encode, MaxEncodedLen};
use hydradx_traits::{evm::EvmAddress, OraclePeriod, RawOracle, Source as EmaSource};
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;
use sp_runtime::DispatchError;
use crate::types::{Balance, PegSource, PegType};

//TODO: rename PegOracle

pub trait PegProvider<AssetId, BlockNumber> {
	type Output;
	type RawOracle: RawOracle<AssetId, Balance, BlockNumber>;
	fn get(peg_asset: AssetId, source: PegSource<AssetId>) -> Result<Self::Output, DispatchError>;
}
pub trait PegOracle<AssetId, Balance, BlockNumber> {
	type Error;
	fn get(source: Source<AssetId>) -> Result<Peg<BlockNumber>, Self::Error>;
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct Peg<BlockNumber> {
	pub val: PegType,
	pub updated_at: BlockNumber,
}

#[derive(Encode, Decode, Eq, PartialEq, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum Source<AssetId> {
	Value(PegType),
	Oracle((EmaSource, OraclePeriod, AssetId, AssetId)),
	MMOracle(EvmAddress),
}

impl<AssetId> From<(PegSource<AssetId>, AssetId)> for Source<AssetId> {
	fn from(item: (PegSource<AssetId>, AssetId)) -> Self {
		return match item.0 {
			PegSource::Value(peg) => Source::Value(peg),
			PegSource::Oracle((source, period, oracle_asset)) => Source::Oracle((source, period, oracle_asset, item.1)),
			PegSource::MMOracle(addr) => Source::MMOracle(addr),
		};
	}
}
