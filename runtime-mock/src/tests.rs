use super::*;
use hydradx_runtime::RegistryStrLimit;
use pallet_asset_registry::{AssetDetails, AssetType};
use sp_runtime::BoundedVec;

fn to_bounded_vec(string: String) -> BoundedVec<u8, RegistryStrLimit> {
	string.into_bytes().to_vec().try_into().unwrap()
}

#[test]
fn asset_registry_setup_should_work() {
	let mut externalities = hydradx_mocked_runtime();

	externalities.execute_with(|| {
		assert_eq!(
			AssetRegistry::assets(0).unwrap(),
			AssetDetails {
				name: Some(to_bounded_vec("HDX".into())),
				asset_type: AssetType::Token,
				existential_deposit: 1_000_000_000_000u128,
				xcm_rate_limit: None,
				symbol: Some(to_bounded_vec("HDX".into())),
				decimals: Some(12),
				is_sufficient: true
			}
		);

		assert_eq!(
			AssetRegistry::assets(1).unwrap(),
			AssetDetails {
				name: Some(to_bounded_vec("Name 1".into())),
				asset_type: AssetType::Token,
				existential_deposit: 1_000u128,
				xcm_rate_limit: None,
				symbol: Some(to_bounded_vec("LRNA".into())),
				decimals: Some(12),
				is_sufficient: true
			}
		);

		assert_eq!(
			AssetRegistry::assets(100).unwrap(),
			AssetDetails {
				name: Some(to_bounded_vec("Name 100".into())),
				asset_type: AssetType::Token,
				existential_deposit: 1_000u128,
				xcm_rate_limit: None,
				symbol: Some(to_bounded_vec("4-Pool".into())),
				decimals: Some(18),
				is_sufficient: true
			}
		);
	});
}
