use sp_std::vec::Vec;

pub trait Registry<AssetId, AssetName, Balance, Error> {
	fn exists(name: AssetId) -> bool;

	fn retrieve_asset(name: &AssetName) -> Result<AssetId, Error>;

	fn retrieve_asset_type(asset_id: AssetId) -> Result<AssetKind, Error>;

	fn create_asset(name: &AssetName, existential_deposit: Balance) -> Result<AssetId, Error>;

	fn get_or_create_asset(name: AssetName, existential_deposit: Balance) -> Result<AssetId, Error> {
		if let Ok(asset_id) = Self::retrieve_asset(&name) {
			Ok(asset_id)
		} else {
			Self::create_asset(&name, existential_deposit)
		}
	}
}

// Use CreateRegistry if possible
pub trait ShareTokenRegistry<AssetId, AssetName, Balance, Error>: Registry<AssetId, AssetName, Balance, Error> {
	fn retrieve_shared_asset(name: &AssetName, assets: &[AssetId]) -> Result<AssetId, Error>;

	fn create_shared_asset(
		name: &AssetName,
		assets: &[AssetId],
		existential_deposit: Balance,
	) -> Result<AssetId, Error>;

	fn get_or_create_shared_asset(
		name: AssetName,
		assets: Vec<AssetId>,
		existential_deposit: Balance,
	) -> Result<AssetId, Error> {
		if let Ok(asset_id) = Self::retrieve_shared_asset(&name, &assets) {
			Ok(asset_id)
		} else {
			Self::create_shared_asset(&name, &assets, existential_deposit)
		}
	}
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum AssetKind {
	Token,
	XYK,
	StableSwap,
	Bond,
	External,
}

pub trait CreateRegistry<AssetId, Balance> {
	type Error;
	fn create_asset(name: &[u8], kind: AssetKind, existential_deposit: Balance) -> Result<AssetId, Self::Error>;
}

// Deprecated.
// TODO: the following macro is commented out for a reason for now - due to failing clippy in CI
// #[deprecated(since = "0.6.0", note = "Please use `AccountIdFor` instead")]
pub trait AssetPairAccountIdFor<AssetId, AccountId> {
	fn from_assets(asset_a: AssetId, asset_b: AssetId, identifier: &str) -> AccountId;
}

/// Abstraction over account id and account name creation for `Assets`
pub trait AccountIdFor<Assets> {
	type AccountId;

	/// Create account id for given assets and an identifier
	fn from_assets(assets: &Assets, identifier: Option<&[u8]>) -> Self::AccountId;

	/// Create a name to uniquely identify a share token for given assets and an identifier.
	fn name(assets: &Assets, identifier: Option<&[u8]>) -> Vec<u8>;
}

use frame_support::dispatch::Parameter;

pub trait Inspect<AssetNativeLocation, Balance> {
	type Error;
	type AssetId: Parameter;
}

#[allow(clippy::too_many_arguments)]
pub trait Create<AssetNativeLocation, Balance>: Inspect<AssetNativeLocation, Balance> {
	fn register_asset(
		asset_id: Option<Self::AssetId>,
		name: Option<&[u8]>,
		kind: AssetKind,
		existential_deposit: Option<Balance>,
		symbol: Option<&[u8]>,
		decimals: Option<u8>,
		location: Option<AssetNativeLocation>,
		xcm_rate_limit: Option<Balance>,
		is_sufficient: bool,
	) -> Result<Self::AssetId, Self::Error>;

	fn register_insufficient_asset(
		asset_id: Option<Self::AssetId>,
		name: Option<&[u8]>,
		kind: AssetKind,
		existential_deposit: Option<Balance>,
		symbol: Option<&[u8]>,
		decimals: Option<u8>,
		location: Option<AssetNativeLocation>,
		xcm_rate_limit: Option<Balance>,
	) -> Result<Self::AssetId, Self::Error> {
        Self::register_asset(asset_id, name, kind, existential_deposit, symbol, decimals, location, xcm_rate_limit, false)
	}
	
    fn register_sufficient_asset(
		asset_id: Option<Self::AssetId>,
		name: Option<&[u8]>,
		kind: AssetKind,
		existential_deposit: Option<Balance>,
		symbol: Option<&[u8]>,
		decimals: Option<u8>,
		location: Option<AssetNativeLocation>,
		xcm_rate_limit: Option<Balance>,
	) -> Result<Self::AssetId, Self::Error> {
        Self::register_asset(asset_id, name, kind, existential_deposit, symbol, decimals, location, xcm_rate_limit, true)
	}
}

pub trait Mutate<AssetNativeLocation, Balance>: Inspect<AssetNativeLocation, Balance> {
	/// Set location for existing asset id if it wasn't set yet.
	fn set_location(asset_id: Self::AssetId, location: AssetNativeLocation) -> Result<(), Self::Error>;

	// /// Set or update location of existing asset
	// fn force_set_location(asset_id: Self::AssetId, location: AssetNativeLocation) -> Result<(, Self::Error)>
}
