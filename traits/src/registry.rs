use sp_std::vec::Vec;

pub trait Registry<AssetId, AssetName, Balance, Error> {
    fn exists(name: AssetId) -> bool;

    fn retrieve_asset(name: &AssetName) -> Result<AssetId, Error>;

    fn create_asset(name: &AssetName, existential_deposit: Balance) -> Result<AssetId, Error>;

    fn get_or_create_asset(name: AssetName, existential_deposit: Balance) -> Result<AssetId, Error> {
        if let Ok(asset_id) = Self::retrieve_asset(&name) {
            Ok(asset_id)
        } else {
            Self::create_asset(&name, existential_deposit)
        }
    }
}

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
