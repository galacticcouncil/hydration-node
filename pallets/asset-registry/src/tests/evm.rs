use super::*;
use frame_support::storage::with_transaction;
use hex_literal::hex;
use polkadot_xcm::v3::Junction::{AccountKey20, Parachain};
use polkadot_xcm::v3::Junctions::{Here, X1, X2};
use polkadot_xcm::v3::{Junction, MultiLocation};
use sp_runtime::{DispatchResult, TransactionOutcome};

use mock::Registry;

fn create_evm_location(address: EvmAddress) -> Option<AssetLocation> {
	Some(AssetLocation(MultiLocation::new(
		0,
		X1(AccountKey20 {
			key: address.into(),
			network: None,
		}),
	)))
}

#[test]
fn ec20_trait_should_return_contract_address() {
	ExtBuilder::default().build().execute_with(|| {
		let _ = with_transaction(|| {
			// Arrange
			assert_ok!(<Registry as Create<Balance>>::register_asset(
				Some(1),
				None,
				AssetKind::Erc20,
				None,
				None,
				None,
				create_evm_location(hex!["77e1733B3163B4455dBE19aC1B26D72D420EEB54"].into()),
				None,
				true
			));

			// Act & Assert
			assert_eq!(
				<Registry as BoundErc20>::contract_address(1),
				Some(hex!["77e1733B3163B4455dBE19aC1B26D72D420EEB54"].into())
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn return_none_when_asset_is_not_erc20() {
	ExtBuilder::default().build().execute_with(|| {
		let _ = with_transaction(|| {
			// Arrange
			assert_ok!(<Registry as Create<Balance>>::register_asset(
				Some(1),
				None,
				AssetKind::Token,
				None,
				None,
				None,
				create_evm_location(hex!["77e1733B3163B4455dBE19aC1B26D72D420EEB54"].into()),
				None,
				true
			));

			// Act & Assert
			assert_eq!(<Registry as BoundErc20>::contract_address(1), None);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn return_zero_when_erc20_has_wrong_location() {
	let locations = vec![
		None,
		Some(AssetLocation(MultiLocation::new(
			0,
			X2(
				Parachain(200),
				Junction::from(BoundedVec::try_from(1_000.encode()).unwrap()),
			),
		))),
		Some(AssetLocation(MultiLocation::new(0, Here))),
	];

	for location in locations {
		ExtBuilder::default().build().execute_with(|| {
			let _ = with_transaction(|| {
				// Arrange
				assert_ok!(<Registry as Create<Balance>>::register_asset(
					Some(1),
					None,
					AssetKind::Erc20,
					None,
					None,
					None,
					location.clone(),
					None,
					true
				));

				// Act & Assert
				assert_eq!(
					<Registry as BoundErc20>::contract_address(1),
					Some(hex!["0000000000000000000000000000000000000000"].into())
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}
}
