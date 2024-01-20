// This file is part of pallet-asset-registry.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::Error;
use crate::mock::AssetId as RegistryAssetId;
use crate::types::{AssetDetails, AssetMetadata, AssetType, Metadata};
use crate::Event;
use crate::{mock::*, XcmRateLimitsInRegistry};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use orml_traits::GetByKey;
use polkadot_xcm::v3::prelude::*;
use sp_std::convert::TryInto;

#[test]
fn register_asset_works() {
	new_test_ext().execute_with(|| {
		let too_long = [1u8; <Test as crate::Config>::StringLimit::get() as usize + 1];

		let ed = 1_000_000u128;

		assert_noop!(
			AssetRegistryPallet::register(
				RuntimeOrigin::root(),
				too_long.to_vec(),
				AssetType::Token,
				ed,
				None,
				None,
				None,
				None
			),
			Error::<Test>::TooLong
		);

		let name: Vec<u8> = b"BSX".to_vec();

		assert_ok!(AssetRegistryPallet::register(
			RuntimeOrigin::root(),
			name.clone(),
			AssetType::Token,
			ed,
			None,
			None,
			None,
			None
		));

		let bn = AssetRegistryPallet::to_bounded_name(name.clone()).unwrap();

		expect_events(vec![Event::Registered {
			asset_id: 1 + SequentialIdStart::get(),
			asset_name: bn.clone(),
			asset_type: AssetType::Token,
		}
		.into()]);

		assert_eq!(
			AssetRegistryPallet::asset_ids(&bn).unwrap(),
			1u32 + SequentialIdStart::get()
		);
		assert_eq!(
			AssetRegistryPallet::assets(1u32 + SequentialIdStart::get()).unwrap(),
			AssetDetails {
				name: bn,
				asset_type: AssetType::Token,
				existential_deposit: ed,
				xcm_rate_limit: None,
			}
		);

		assert_noop!(
			AssetRegistryPallet::register(
				RuntimeOrigin::root(),
				name,
				AssetType::Token,
				ed,
				None,
				None,
				None,
				None
			),
			Error::<Test>::AssetAlreadyRegistered
		);
	});
}

#[test]
fn create_asset() {
	new_test_ext().execute_with(|| {
		let ed = 1_000_000u128;

		assert_ok!(AssetRegistryPallet::get_or_create_asset(
			b"BSX".to_vec(),
			AssetType::Token,
			ed,
			None,
		));

		let dot_asset = AssetRegistryPallet::get_or_create_asset(b"DOT".to_vec(), AssetType::Token, ed, None);
		assert_ok!(dot_asset);
		let dot_asset_id = dot_asset.ok().unwrap();

		assert_ok!(AssetRegistryPallet::get_or_create_asset(
			b"BTC".to_vec(),
			AssetType::Token,
			ed,
			None,
		));

		let current_asset_id = AssetRegistryPallet::next_asset_id().unwrap();

		// Existing asset should return previously created one.
		assert_ok!(
			AssetRegistryPallet::get_or_create_asset(b"DOT".to_vec(), AssetType::Token, ed, None),
			dot_asset_id
		);

		// Retrieving existing asset should not increased the next asset id counter.
		assert_eq!(AssetRegistryPallet::next_asset_id().unwrap(), current_asset_id);

		let dot: BoundedVec<u8, <Test as crate::Config>::StringLimit> = b"DOT".to_vec().try_into().unwrap();
		let aaa: BoundedVec<u8, <Test as crate::Config>::StringLimit> = b"AAA".to_vec().try_into().unwrap();

		assert_eq!(
			AssetRegistryPallet::asset_ids(dot).unwrap(),
			2u32 + SequentialIdStart::get()
		);
		assert!(AssetRegistryPallet::asset_ids(aaa).is_none());
	});
}

#[test]
fn location_mapping_works() {
	new_test_ext().execute_with(|| {
		let bn = AssetRegistryPallet::to_bounded_name(b"BSX".to_vec()).unwrap();

		let ed = 1_000_000u128;

		assert_ok!(AssetRegistryPallet::get_or_create_asset(
			b"BSX".to_vec(),
			AssetType::Token,
			ed,
			None,
		));
		let asset_id: RegistryAssetId =
			AssetRegistryPallet::get_or_create_asset(b"BSX".to_vec(), AssetType::Token, ed, None).unwrap();

		crate::Assets::<Test>::insert(
			asset_id,
			AssetDetails::<RegistryAssetId, Balance, BoundedVec<u8, RegistryStringLimit>> {
				name: bn,
				asset_type: AssetType::Token,
				existential_deposit: ed,
				xcm_rate_limit: None,
			},
		);

		let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
		let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

		assert_ok!(AssetRegistryPallet::set_location(
			RuntimeOrigin::root(),
			asset_id,
			asset_location.clone()
		));

		expect_events(vec![Event::LocationSet {
			asset_id: 1 + SequentialIdStart::get(),
			location: asset_location.clone(),
		}
		.into()]);

		assert_eq!(
			AssetRegistryPallet::location_to_asset(asset_location.clone()),
			Some(asset_id)
		);
		assert_eq!(
			AssetRegistryPallet::asset_to_location(asset_id),
			Some(asset_location.clone())
		);

		// asset location for the native asset cannot be changed
		assert_noop!(
			AssetRegistryPallet::set_location(
				RuntimeOrigin::root(),
				<Test as crate::Config>::NativeAssetId::get(),
				asset_location
			),
			Error::<Test>::CannotUpdateLocation
		);
	});
}

#[test]
fn genesis_config_works() {
	ExtBuilder::default()
		.with_native_asset_name(b"NATIVE".to_vec())
		.build()
		.execute_with(|| {
			let native: BoundedVec<u8, <Test as crate::Config>::StringLimit> = b"NATIVE".to_vec().try_into().unwrap();
			assert_eq!(AssetRegistryPallet::asset_ids(native).unwrap(), 0u32);
		});

	let one = b"ONE".to_vec();
	let life = b"LIFE".to_vec();

	ExtBuilder::default()
		.with_assets(vec![
			(one.clone(), 1_000u128, None),
			(life.clone(), 1_000u128, Some(42)),
		])
		.build()
		.execute_with(|| {
			let native: BoundedVec<u8, <Test as crate::Config>::StringLimit> = b"NATIVE".to_vec().try_into().unwrap();
			assert_eq!(AssetRegistryPallet::asset_ids(native), None);

			let bsx: BoundedVec<u8, <Test as crate::Config>::StringLimit> = b"HDX".to_vec().try_into().unwrap();
			assert_eq!(AssetRegistryPallet::asset_ids(bsx).unwrap(), 0u32);

			let one: BoundedVec<u8, <Test as crate::Config>::StringLimit> = one.try_into().unwrap();
			assert_eq!(
				AssetRegistryPallet::asset_ids(one.clone()).unwrap(),
				1u32 + SequentialIdStart::get()
			);
			assert_eq!(
				AssetRegistryPallet::assets(1u32 + SequentialIdStart::get()).unwrap(),
				AssetDetails {
					name: one,
					asset_type: AssetType::Token,
					existential_deposit: 1_000u128,
					xcm_rate_limit: None,
				}
			);

			let life: BoundedVec<u8, <Test as crate::Config>::StringLimit> = life.try_into().unwrap();
			assert_eq!(AssetRegistryPallet::asset_ids(life.clone()).unwrap(), 42u32);
			assert_eq!(
				AssetRegistryPallet::assets(42u32).unwrap(),
				AssetDetails {
					name: life,
					asset_type: AssetType::Token,
					existential_deposit: 1_000u128,
					xcm_rate_limit: None,
				}
			);
		});
}

#[test]
fn set_metadata_works() {
	ExtBuilder::default()
		.with_assets(vec![(b"DOT".to_vec(), 1_000u128, None)])
		.build()
		.execute_with(|| {
			System::set_block_number(1); //TO have the ement emitted

			let dot: BoundedVec<u8, <Test as crate::Config>::StringLimit> = b"DOT".to_vec().try_into().unwrap();
			let dot_id = AssetRegistryPallet::asset_ids(dot).unwrap();
			let b_symbol: BoundedVec<u8, <Test as crate::Config>::StringLimit> = b"xDOT".to_vec().try_into().unwrap();

			assert_ok!(AssetRegistryPallet::set_metadata(
				RuntimeOrigin::root(),
				dot_id,
				b"xDOT".to_vec(),
				12u8
			));

			expect_events(vec![Event::MetadataSet {
				asset_id: dot_id,
				symbol: b_symbol.clone(),
				decimals: 12u8,
			}
			.into()]);

			assert_eq!(
				AssetRegistryPallet::asset_metadata(dot_id).unwrap(),
				AssetMetadata {
					decimals: 12u8,
					symbol: b_symbol.clone(),
				}
			);

			assert_ok!(AssetRegistryPallet::set_metadata(
				RuntimeOrigin::root(),
				dot_id,
				b"xDOT".to_vec(),
				30u8
			));

			assert_eq!(
				AssetRegistryPallet::asset_metadata(dot_id).unwrap(),
				AssetMetadata {
					decimals: 30u8,
					symbol: b_symbol,
				}
			);

			assert_noop!(
				AssetRegistryPallet::set_metadata(RuntimeOrigin::root(), dot_id, b"JUST_TOO_LONG".to_vec(), 30u8),
				Error::<Test>::TooLong
			);

			assert_noop!(
				AssetRegistryPallet::set_metadata(RuntimeOrigin::root(), 100, b"NONE".to_vec(), 30u8),
				Error::<Test>::AssetNotFound
			);
		});
}

#[test]
fn update_asset() {
	new_test_ext().execute_with(|| {
		let ed = 1_000_000u128;

		let btc_asset_id: RegistryAssetId =
			AssetRegistryPallet::get_or_create_asset(b"BTC".to_vec(), AssetType::Token, ed, None).unwrap();
		let usd_asset_id: RegistryAssetId =
			AssetRegistryPallet::get_or_create_asset(b"USD".to_vec(), AssetType::Token, ed, None).unwrap();

		let next_asset_id = AssetRegistryPallet::next_asset_id().unwrap();

		// set a new name and type for an existing asset
		assert_ok!(AssetRegistryPallet::update(
			RuntimeOrigin::root(),
			btc_asset_id,
			b"superBTC".to_vec(),
			AssetType::Token,
			None,
			None,
		));
		let bn = AssetRegistryPallet::to_bounded_name(b"superBTC".to_vec()).unwrap();

		expect_events(vec![Event::Updated {
			asset_id: btc_asset_id,
			asset_name: bn.clone(),
			asset_type: AssetType::Token,
			existential_deposit: 1_000_000,
			xcm_rate_limit: None,
		}
		.into()]);

		assert_eq!(
			AssetRegistryPallet::assets(btc_asset_id).unwrap(),
			AssetDetails {
				name: bn,
				asset_type: AssetType::Token,
				existential_deposit: ed,
				xcm_rate_limit: None,
			}
		);

		let new_btc_name: BoundedVec<u8, <Test as crate::Config>::StringLimit> =
			b"superBTC".to_vec().try_into().unwrap();
		assert_eq!(
			AssetRegistryPallet::asset_ids(new_btc_name).unwrap(),
			1u32 + SequentialIdStart::get()
		);

		// cannot set existing name for an existing asset
		assert_noop!(
			(AssetRegistryPallet::update(
				RuntimeOrigin::root(),
				usd_asset_id,
				b"superBTC".to_vec(),
				AssetType::Token,
				None,
				None,
			)),
			Error::<Test>::AssetAlreadyRegistered
		);

		// cannot set a new name for a non-existent asset
		assert_noop!(
			(AssetRegistryPallet::update(
				RuntimeOrigin::root(),
				next_asset_id,
				b"VOID".to_vec(),
				AssetType::Token,
				None,
				None,
			)),
			Error::<Test>::AssetNotFound
		);

		// corner case: change the name and also the type for an existing asset (token -> pool share)
		assert_ok!(AssetRegistryPallet::update(
			RuntimeOrigin::root(),
			btc_asset_id,
			b"BTCUSD".to_vec(),
			AssetType::PoolShare(btc_asset_id, usd_asset_id),
			None,
			None,
		));

		// Update ED
		assert_ok!(AssetRegistryPallet::update(
			RuntimeOrigin::root(),
			btc_asset_id,
			b"BTCUSD".to_vec(),
			AssetType::PoolShare(btc_asset_id, usd_asset_id),
			Some(1_234_567u128),
			None,
		));

		let btcusd = AssetRegistryPallet::to_bounded_name(b"BTCUSD".to_vec()).unwrap();

		assert_eq!(
			AssetRegistryPallet::assets(btc_asset_id).unwrap(),
			AssetDetails {
				name: btcusd,
				asset_type: AssetType::PoolShare(btc_asset_id, usd_asset_id),
				existential_deposit: 1_234_567u128,
				xcm_rate_limit: None,
			}
		);

		// corner case: change the name and also the type for an existing asset (pool share -> token)
		assert_ok!(AssetRegistryPallet::update(
			RuntimeOrigin::root(),
			btc_asset_id,
			b"superBTC".to_vec(),
			AssetType::Token,
			None,
			None,
		));

		let superbtc_name: BoundedVec<u8, <Test as crate::Config>::StringLimit> =
			b"superBTC".to_vec().try_into().unwrap();

		assert_eq!(
			AssetRegistryPallet::assets(1u32 + SequentialIdStart::get()).unwrap(),
			AssetDetails {
				name: superbtc_name,
				asset_type: AssetType::Token,
				existential_deposit: 1_234_567u128,
				xcm_rate_limit: None,
			}
		);
	});
}

#[test]
fn update_should_update_xcm_rate_limit() {
	new_test_ext().execute_with(|| {
		let ed = 1_000_000u128;

		let btc_asset_id: RegistryAssetId =
			AssetRegistryPallet::get_or_create_asset(b"BTC".to_vec(), AssetType::Token, ed, None).unwrap();

		assert_ok!(AssetRegistryPallet::update(
			RuntimeOrigin::root(),
			btc_asset_id,
			b"superBTC".to_vec(),
			AssetType::Token,
			None,
			Some(1000 * UNIT)
		));

		let bn = AssetRegistryPallet::to_bounded_name(b"superBTC".to_vec()).unwrap();

		assert_eq!(
			AssetRegistryPallet::assets(btc_asset_id).unwrap(),
			AssetDetails {
				name: bn.clone(),
				asset_type: AssetType::Token,
				existential_deposit: ed,
				xcm_rate_limit: Some(1000 * UNIT),
			}
		);

		assert_eq!(XcmRateLimitsInRegistry::<Test>::get(&btc_asset_id), Some(1000 * UNIT));

		expect_events(vec![Event::Updated {
			asset_id: btc_asset_id,
			asset_name: bn,
			asset_type: AssetType::Token,
			existential_deposit: ed,
			xcm_rate_limit: Some(1000 * UNIT),
		}
		.into()]);
	});
}

#[test]
fn get_ed_by_key_works() {
	ExtBuilder::default()
		.with_native_asset_name(b"NATIVE".to_vec())
		.with_assets(vec![
			(b"ONE".to_vec(), 1_000u128, None),
			(b"TWO".to_vec(), 2_000u128, None),
		])
		.build()
		.execute_with(|| {
			assert_eq!(AssetRegistryPallet::get(&(1u32 + SequentialIdStart::get())), 1_000u128);
			assert_eq!(AssetRegistryPallet::get(&(2u32 + SequentialIdStart::get())), 2_000u128);
			assert_eq!(AssetRegistryPallet::get(&0u32), 1_000_000u128);
			assert_eq!(
				AssetRegistryPallet::get(&(1_000u32 + SequentialIdStart::get())),
				Balance::MAX
			); // Non-existing assets are not supported
		});
}

#[test]
fn register_asset_should_work_when_asset_is_provided() {
	ExtBuilder::default()
		.with_native_asset_name(b"NATIVE".to_vec())
		.build()
		.execute_with(|| {
			assert_ok!(AssetRegistryPallet::register(
				RuntimeOrigin::root(),
				b"asset_id".to_vec(),
				AssetType::Token,
				1_000_000,
				Some(1u32),
				None,
				None,
				None
			),);

			let bn = AssetRegistryPallet::to_bounded_name(b"asset_id".to_vec()).unwrap();
			assert_eq!(
				AssetRegistryPallet::assets(1u32).unwrap(),
				AssetDetails {
					name: bn,
					asset_type: AssetType::Token,
					existential_deposit: 1_000_000,
					xcm_rate_limit: None,
				}
			);
		});
}

#[test]
fn register_asset_should_fail_when_provided_asset_is_native_asset() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AssetRegistryPallet::register(
				RuntimeOrigin::root(),
				b"asset_id".to_vec(),
				AssetType::Token,
				1_000_000,
				Some(NativeAssetId::get()),
				None,
				None,
				None
			),
			Error::<Test>::AssetAlreadyRegistered
		);
	});
}

#[test]
fn register_asset_should_fail_when_provided_asset_is_already_registered() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AssetRegistryPallet::register(
			RuntimeOrigin::root(),
			b"asset_id".to_vec(),
			AssetType::Token,
			1_000_000,
			Some(10),
			None,
			None,
			None
		));
		assert_noop!(
			AssetRegistryPallet::register(
				RuntimeOrigin::root(),
				b"asset_id_2".to_vec(),
				AssetType::Token,
				1_000_000,
				Some(10),
				None,
				None,
				None
			),
			Error::<Test>::AssetAlreadyRegistered
		);
	});
}

#[test]
fn register_asset_should_fail_when_provided_asset_is_outside_reserved_range() {
	ExtBuilder::default()
		.with_native_asset_name(b"NATIVE".to_vec())
		.build()
		.execute_with(|| {
			assert_noop!(
				AssetRegistryPallet::register(
					RuntimeOrigin::root(),
					b"asset_id".to_vec(),
					AssetType::Token,
					1_000_000,
					Some(SequentialIdStart::get()),
					None,
					None,
					None
				),
				Error::<Test>::NotInReservedRange
			);

			assert_noop!(
				AssetRegistryPallet::register(
					RuntimeOrigin::root(),
					b"asset_id".to_vec(),
					AssetType::Token,
					1_000_000,
					Some(SequentialIdStart::get() + 100),
					None,
					None,
					None
				),
				Error::<Test>::NotInReservedRange
			);
		});
}

#[test]
fn register_asset_should_work_when_metadata_is_provided() {
	new_test_ext().execute_with(|| {
		let asset_id: RegistryAssetId = 10;
		let decimals = 18;
		let symbol = b"SYM".to_vec();
		let asset_name = b"asset_name".to_vec();
		let b_symbol = AssetRegistryPallet::to_bounded_name(symbol.clone()).unwrap();
		let b_asset_name = AssetRegistryPallet::to_bounded_name(asset_name.clone()).unwrap();

		assert_ok!(AssetRegistryPallet::register(
			RuntimeOrigin::root(),
			asset_name,
			AssetType::Token,
			1_000_000,
			Some(asset_id),
			Some(Metadata { symbol, decimals }),
			None,
			None,
		));

		expect_events(vec![
			Event::Registered {
				asset_id,
				asset_name: b_asset_name.clone(),
				asset_type: AssetType::Token,
			}
			.into(),
			Event::MetadataSet {
				asset_id,
				symbol: b_symbol.clone(),
				decimals,
			}
			.into(),
		]);

		assert_eq!(
			AssetRegistryPallet::assets(asset_id).unwrap(),
			AssetDetails {
				name: b_asset_name,
				asset_type: AssetType::Token,
				existential_deposit: 1_000_000,
				xcm_rate_limit: None,
			}
		);

		assert_eq!(
			AssetRegistryPallet::asset_metadata(asset_id).unwrap(),
			AssetMetadata {
				decimals: 18u8,
				symbol: b_symbol,
			}
		);
	});
}

#[test]
fn register_asset_should_work_when_location_is_provided() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id: RegistryAssetId = 10;

		let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
		let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

		assert_ok!(AssetRegistryPallet::register(
			RuntimeOrigin::root(),
			b"asset_id".to_vec(),
			AssetType::Token,
			1_000_000,
			Some(asset_id),
			None,
			Some(asset_location.clone()),
			None
		),);

		let bn = AssetRegistryPallet::to_bounded_name(b"asset_id".to_vec()).unwrap();
		assert_eq!(
			AssetRegistryPallet::assets(asset_id).unwrap(),
			AssetDetails {
				name: bn,
				asset_type: AssetType::Token,
				existential_deposit: 1_000_000,
				xcm_rate_limit: None,
			}
		);
		assert_eq!(
			AssetRegistryPallet::location_to_asset(asset_location.clone()),
			Some(asset_id)
		);
		assert_eq!(AssetRegistryPallet::asset_to_location(asset_id), Some(asset_location));

		assert!(AssetRegistryPallet::asset_metadata(asset_id).is_none(),);
	});
}

#[test]
fn register_asset_should_fail_when_location_is_already_registered() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let asset_id: RegistryAssetId = 10;
		let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
		let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(2021), key)));
		assert_ok!(AssetRegistryPallet::register(
			RuntimeOrigin::root(),
			b"asset_id".to_vec(),
			AssetType::Token,
			1_000_000,
			Some(asset_id),
			None,
			Some(asset_location.clone()),
			None
		),);

		// Act & Assert
		assert_noop!(
			AssetRegistryPallet::register(
				RuntimeOrigin::root(),
				b"asset_id_2".to_vec(),
				AssetType::Token,
				1_000_000,
				Some(asset_id + 1),
				None,
				Some(asset_location),
				None
			),
			Error::<Test>::LocationAlreadyRegistered
		);
	});
}

#[test]
fn set_location_should_fail_when_location_is_already_registered() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let asset_id: RegistryAssetId = 10;
		let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
		let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(2021), key)));
		assert_ok!(AssetRegistryPallet::register(
			RuntimeOrigin::root(),
			b"asset_id".to_vec(),
			AssetType::Token,
			1_000_000,
			Some(asset_id),
			None,
			Some(asset_location.clone()),
			None
		),);

		// Act & Assert
		assert_noop!(
			AssetRegistryPallet::set_location(RuntimeOrigin::root(), asset_id, asset_location),
			Error::<Test>::LocationAlreadyRegistered
		);
	});
}

#[test]
fn set_location_should_remove_old_location() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let asset_id: RegistryAssetId = 10;
		let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
		let old_asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(2021), key)));
		assert_ok!(AssetRegistryPallet::register(
			RuntimeOrigin::root(),
			b"asset_id".to_vec(),
			AssetType::Token,
			1_000_000,
			Some(asset_id),
			None,
			Some(old_asset_location.clone()),
			None
		),);

		// Act
		assert_ok!(AssetRegistryPallet::set_location(
			RuntimeOrigin::root(),
			asset_id,
			AssetLocation(MultiLocation::new(0, X2(Parachain(2022), key)))
		));

		// Assert
		assert_eq!(AssetRegistryPallet::location_to_asset(old_asset_location), None);
	});
}

#[test]
fn register_asset_should_work_when_all_optional_are_provided() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id: RegistryAssetId = 10;

		let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
		let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

		assert_ok!(AssetRegistryPallet::register(
			RuntimeOrigin::root(),
			b"asset_id".to_vec(),
			AssetType::Token,
			1_000_000,
			Some(asset_id),
			Some(Metadata {
				symbol: b"SYM".to_vec(),
				decimals: 18
			}),
			Some(asset_location.clone()),
			Some(1000 * UNIT)
		),);

		let bn = AssetRegistryPallet::to_bounded_name(b"asset_id".to_vec()).unwrap();
		assert_eq!(
			AssetRegistryPallet::assets(asset_id).unwrap(),
			AssetDetails {
				name: bn,
				asset_type: AssetType::Token,
				existential_deposit: 1_000_000,
				xcm_rate_limit: Some(1000 * UNIT),
			}
		);
		assert_eq!(
			AssetRegistryPallet::location_to_asset(asset_location.clone()),
			Some(asset_id)
		);
		assert_eq!(AssetRegistryPallet::asset_to_location(asset_id), Some(asset_location));
		let b_symbol: BoundedVec<u8, <Test as crate::Config>::StringLimit> = b"SYM".to_vec().try_into().unwrap();
		assert_eq!(
			AssetRegistryPallet::asset_metadata(asset_id).unwrap(),
			AssetMetadata {
				decimals: 18u8,
				symbol: b_symbol,
			}
		);
	});
}
