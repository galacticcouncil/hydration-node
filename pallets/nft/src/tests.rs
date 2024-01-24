// This file is part of galacticcouncil/warehouse.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use frame_support::{assert_noop, assert_ok, traits::tokens::nonfungibles::*};

use super::*;
use mock::*;
use std::convert::TryInto;
type NFTPallet = Pallet<Test>;

#[test]
fn create_collection_works() {
	ExtBuilder.build().execute_with(|| {
		let metadata: BoundedVec<u8, <Test as pallet_uniques::Config>::StringLimit> =
			b"metadata".to_vec().try_into().unwrap();

		assert_ok!(NFTPallet::create_collection(
			RuntimeOrigin::signed(ALICE),
			COLLECTION_ID_0,
			Default::default(), // Marketplace
			metadata.clone()
		));
		assert_eq!(
			NFTPallet::collections(COLLECTION_ID_0).unwrap(),
			CollectionInfo {
				collection_type: CollectionType::Marketplace,
				metadata: metadata.clone()
			}
		);

		expect_events(vec![crate::Event::CollectionCreated {
			owner: ALICE,
			collection_id: COLLECTION_ID_0,
			collection_type: CollectionType::Marketplace,
			metadata: metadata.clone(),
		}
		.into()]);

		// not allowed in Permissions
		assert_noop!(
			NFTPallet::create_collection(
				RuntimeOrigin::signed(ALICE),
				COLLECTION_ID_2,
				CollectionType::LiquidityMining,
				metadata.clone()
			),
			Error::<Test>::NotPermitted
		);

		// existing collection ID
		assert_noop!(
			NFTPallet::create_collection(
				RuntimeOrigin::signed(ALICE),
				COLLECTION_ID_0,
				CollectionType::Marketplace,
				metadata.clone()
			),
			pallet_uniques::Error::<Test>::InUse
		);

		// reserved collection ID
		assert_noop!(
			NFTPallet::create_collection(
				RuntimeOrigin::signed(ALICE),
				COLLECTION_ID_RESERVED,
				CollectionType::Marketplace,
				metadata
			),
			Error::<Test>::IdReserved
		);
	})
}

#[test]
fn mint_works() {
	ExtBuilder.build().execute_with(|| {
		let metadata: BoundedVec<u8, <Test as pallet_uniques::Config>::StringLimit> =
			b"metadata".to_vec().try_into().unwrap();

		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_0,
			CollectionType::Marketplace,
			metadata.clone()
		));
		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_1,
			CollectionType::LiquidityMining,
			metadata.clone()
		));

		assert_ok!(NFTPallet::mint(
			RuntimeOrigin::signed(ALICE),
			COLLECTION_ID_0,
			ITEM_ID_0,
			metadata.clone()
		));
		assert_eq!(
			NFTPallet::items(COLLECTION_ID_0, ITEM_ID_0).unwrap(),
			ItemInfo {
				metadata: metadata.clone()
			}
		);

		expect_events(vec![crate::Event::ItemMinted {
			owner: ALICE,
			collection_id: COLLECTION_ID_0,
			item_id: ITEM_ID_0,
			metadata: metadata.clone(),
		}
		.into()]);

		// duplicate item
		assert_noop!(
			NFTPallet::mint(
				RuntimeOrigin::signed(ALICE),
				COLLECTION_ID_0,
				ITEM_ID_0,
				metadata.clone()
			),
			pallet_uniques::Error::<Test>::AlreadyExists
		);

		// not allowed in Permissions
		assert_noop!(
			NFTPallet::mint(
				RuntimeOrigin::signed(ALICE),
				COLLECTION_ID_1,
				ITEM_ID_0,
				metadata.clone()
			),
			Error::<Test>::NotPermitted
		);

		// not owner
		assert_noop!(
			NFTPallet::mint(RuntimeOrigin::signed(BOB), COLLECTION_ID_0, ITEM_ID_1, metadata.clone()),
			Error::<Test>::NotPermitted
		);

		// invalid collection ID
		assert_noop!(
			NFTPallet::mint(
				RuntimeOrigin::signed(ALICE),
				NON_EXISTING_COLLECTION_ID,
				ITEM_ID_0,
				metadata
			),
			Error::<Test>::CollectionUnknown
		);
	});
}

#[test]
fn transfer_works() {
	ExtBuilder.build().execute_with(|| {
		let metadata: BoundedVec<u8, <Test as pallet_uniques::Config>::StringLimit> =
			b"metadata".to_vec().try_into().unwrap();

		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_0,
			CollectionType::Marketplace,
			metadata.clone()
		));
		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_1,
			CollectionType::LiquidityMining,
			metadata.clone()
		));
		assert_ok!(NFTPallet::do_mint(ALICE, COLLECTION_ID_0, ITEM_ID_0, metadata.clone()));
		assert_ok!(NFTPallet::do_mint(ALICE, COLLECTION_ID_1, ITEM_ID_0, metadata));

		// not existing
		assert_noop!(
			NFTPallet::transfer(RuntimeOrigin::signed(CHARLIE), COLLECTION_ID_2, ITEM_ID_0, ALICE),
			Error::<Test>::CollectionUnknown
		);

		// not owner
		assert_noop!(
			NFTPallet::transfer(RuntimeOrigin::signed(CHARLIE), COLLECTION_ID_0, ITEM_ID_0, ALICE),
			Error::<Test>::NotPermitted
		);

		// not allowed in Permissions
		assert_noop!(
			NFTPallet::transfer(RuntimeOrigin::signed(ALICE), COLLECTION_ID_1, ITEM_ID_0, BOB),
			Error::<Test>::NotPermitted
		);

		assert_ok!(NFTPallet::transfer(
			RuntimeOrigin::signed(ALICE),
			COLLECTION_ID_0,
			ITEM_ID_0,
			ALICE
		));
		assert_eq!(NFTPallet::owner(&COLLECTION_ID_0, &ITEM_ID_0).unwrap(), ALICE);

		assert_ok!(NFTPallet::transfer(
			RuntimeOrigin::signed(ALICE),
			COLLECTION_ID_0,
			ITEM_ID_0,
			BOB
		));
		assert_eq!(NFTPallet::owner(&COLLECTION_ID_0, &ITEM_ID_0).unwrap(), BOB);

		expect_events(vec![crate::Event::ItemTransferred {
			from: ALICE,
			to: BOB,
			collection_id: COLLECTION_ID_0,
			item_id: ITEM_ID_0,
		}
		.into()]);
	});
}

#[test]
fn burn_works() {
	ExtBuilder.build().execute_with(|| {
		let metadata: BoundedVec<u8, <Test as pallet_uniques::Config>::StringLimit> =
			b"metadata".to_vec().try_into().unwrap();

		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_0,
			CollectionType::Marketplace,
			metadata.clone()
		));
		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_1,
			CollectionType::LiquidityMining,
			metadata.clone()
		));
		assert_ok!(NFTPallet::do_mint(ALICE, COLLECTION_ID_0, ITEM_ID_0, metadata.clone()));
		assert_ok!(NFTPallet::do_mint(ALICE, COLLECTION_ID_1, ITEM_ID_0, metadata));

		// not owner
		assert_noop!(
			NFTPallet::burn(RuntimeOrigin::signed(BOB), COLLECTION_ID_0, ITEM_ID_0),
			Error::<Test>::NotPermitted
		);

		// not allowed in Permissions
		assert_noop!(
			NFTPallet::burn(RuntimeOrigin::signed(ALICE), COLLECTION_ID_1, ITEM_ID_0),
			Error::<Test>::NotPermitted
		);

		assert_ok!(NFTPallet::burn(
			RuntimeOrigin::signed(ALICE),
			COLLECTION_ID_0,
			ITEM_ID_0
		));
		assert!(!<Items<Test>>::contains_key(COLLECTION_ID_0, ITEM_ID_0));

		expect_events(vec![crate::Event::ItemBurned {
			owner: ALICE,
			collection_id: COLLECTION_ID_0,
			item_id: ITEM_ID_0,
		}
		.into()]);

		// not existing
		assert_noop!(
			NFTPallet::burn(RuntimeOrigin::signed(ALICE), COLLECTION_ID_0, ITEM_ID_0),
			Error::<Test>::ItemUnknown
		);
	});
}

#[test]
fn destroy_collection_works() {
	ExtBuilder.build().execute_with(|| {
		let metadata: BoundedVec<u8, <Test as pallet_uniques::Config>::StringLimit> =
			b"metadata".to_vec().try_into().unwrap();

		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_0,
			CollectionType::Marketplace,
			metadata.clone()
		));
		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_1,
			CollectionType::LiquidityMining,
			metadata.clone()
		));
		assert_ok!(NFTPallet::do_mint(ALICE, COLLECTION_ID_0, ITEM_ID_0, metadata));

		// existing item
		assert_noop!(
			NFTPallet::destroy_collection(RuntimeOrigin::signed(ALICE), COLLECTION_ID_0),
			Error::<Test>::TokenCollectionNotEmpty
		);
		assert_ok!(NFTPallet::do_burn(ALICE, COLLECTION_ID_0, ITEM_ID_0));

		// not allowed in Permissions
		assert_noop!(
			NFTPallet::destroy_collection(RuntimeOrigin::signed(ALICE), COLLECTION_ID_1),
			Error::<Test>::NotPermitted
		);

		// not owner
		assert_noop!(
			NFTPallet::destroy_collection(RuntimeOrigin::signed(CHARLIE), COLLECTION_ID_0),
			pallet_uniques::Error::<Test>::NoPermission
		);

		assert_ok!(NFTPallet::destroy_collection(
			RuntimeOrigin::signed(ALICE),
			COLLECTION_ID_0
		));
		assert_eq!(NFTPallet::collections(COLLECTION_ID_0), None);

		expect_events(vec![crate::Event::CollectionDestroyed {
			owner: ALICE,
			collection_id: COLLECTION_ID_0,
		}
		.into()]);

		// not existing
		assert_noop!(
			NFTPallet::destroy_collection(RuntimeOrigin::signed(ALICE), COLLECTION_ID_0),
			Error::<Test>::CollectionUnknown
		);
	});
}

#[test]
fn deposit_works() {
	ExtBuilder.build().execute_with(|| {
		let metadata: BoundedVec<u8, <Test as pallet_uniques::Config>::StringLimit> =
			b"metadata".to_vec().try_into().unwrap();

		let collection_deposit = <Test as pallet_uniques::Config>::CollectionDeposit::get();
		let initial_balance = <Test as pallet_uniques::Config>::Currency::free_balance(&ALICE);

		// has deposit
		assert_eq!(<Test as pallet_uniques::Config>::Currency::reserved_balance(&ALICE), 0);
		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_0,
			CollectionType::Marketplace,
			metadata.clone()
		));
		assert_eq!(
			<Test as pallet_uniques::Config>::Currency::free_balance(&ALICE),
			initial_balance - collection_deposit
		);
		assert_eq!(
			<Test as pallet_uniques::Config>::Currency::reserved_balance(&ALICE),
			collection_deposit
		);

		assert_ok!(NFTPallet::do_destroy_collection(ALICE, COLLECTION_ID_0));
		assert_eq!(
			<Test as pallet_uniques::Config>::Currency::free_balance(&ALICE),
			initial_balance
		);
		assert_eq!(<Test as pallet_uniques::Config>::Currency::reserved_balance(&ALICE), 0);

		// no deposit
		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_0,
			CollectionType::LiquidityMining,
			metadata
		));
		assert_eq!(
			<Test as pallet_uniques::Config>::Currency::free_balance(&ALICE),
			initial_balance
		);
		assert_eq!(<Test as pallet_uniques::Config>::Currency::reserved_balance(&ALICE), 0);

		assert_ok!(NFTPallet::do_destroy_collection(ALICE, COLLECTION_ID_0));
		assert_eq!(
			<Test as pallet_uniques::Config>::Currency::free_balance(&ALICE),
			initial_balance
		);
		assert_eq!(<Test as pallet_uniques::Config>::Currency::reserved_balance(&ALICE), 0);
	})
}

#[test]
fn inspect_trait_should_work() {
	ExtBuilder.build().execute_with(|| {
		let metadata: BoundedVec<u8, <Test as pallet_uniques::Config>::StringLimit> =
			b"metadata".to_vec().try_into().unwrap();

		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_0,
			CollectionType::Marketplace,
			metadata.clone()
		));

		assert_ok!(NFTPallet::do_mint(ALICE, COLLECTION_ID_0, ITEM_ID_0, metadata));

		assert_eq!(
			<NFTPallet as Inspect<<Test as frame_system::Config>::AccountId>>::owner(&COLLECTION_ID_0, &ITEM_ID_0),
			Some(ALICE)
		);
		assert_eq!(
			<NFTPallet as Inspect<<Test as frame_system::Config>::AccountId>>::owner(&COLLECTION_ID_1, &ITEM_ID_0),
			None
		);
		assert_eq!(
			<NFTPallet as Inspect<<Test as frame_system::Config>::AccountId>>::owner(&COLLECTION_ID_0, &ITEM_ID_1),
			None
		);

		assert_eq!(
			<NFTPallet as Inspect<<Test as frame_system::Config>::AccountId>>::collection_owner(&COLLECTION_ID_0),
			Some(ALICE)
		);
		assert_eq!(
			<NFTPallet as Inspect<<Test as frame_system::Config>::AccountId>>::collection_owner(&COLLECTION_ID_1),
			None
		);

		assert!(
			<NFTPallet as Inspect<<Test as frame_system::Config>::AccountId>>::can_transfer(
				&COLLECTION_ID_0,
				&ITEM_ID_0
			)
		);
		assert!(
			!<NFTPallet as Inspect<<Test as frame_system::Config>::AccountId>>::can_transfer(
				&COLLECTION_ID_1,
				&ITEM_ID_1
			)
		);
	});
}

#[test]
fn inspect_enumerable_trait_should_work() {
	ExtBuilder.build().execute_with(|| {
		let metadata: BoundedVec<u8, <Test as pallet_uniques::Config>::StringLimit> =
			b"metadata".to_vec().try_into().unwrap();

		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_0,
			CollectionType::Marketplace,
			metadata.clone()
		));

		assert_ok!(NFTPallet::do_mint(ALICE, COLLECTION_ID_0, ITEM_ID_0, metadata));

		assert_eq!(
			*<NFTPallet as InspectEnumerable<<Test as frame_system::Config>::AccountId>>::collections()
				.collect::<Vec<CollectionId>>(),
			vec![COLLECTION_ID_0]
		);
		assert_eq!(
			*<NFTPallet as InspectEnumerable<<Test as frame_system::Config>::AccountId>>::items(&COLLECTION_ID_0)
				.collect::<Vec<ItemId>>(),
			vec![ITEM_ID_0]
		);
		assert_eq!(
			*<NFTPallet as InspectEnumerable<<Test as frame_system::Config>::AccountId>>::owned(&ALICE)
				.collect::<Vec<(CollectionId, ItemId)>>(),
			vec![(COLLECTION_ID_0, ITEM_ID_0)]
		);
		assert_eq!(
			*<NFTPallet as InspectEnumerable<<Test as frame_system::Config>::AccountId>>::owned_in_collection(
				&COLLECTION_ID_0,
				&ALICE
			)
			.collect::<Vec<ItemId>>(),
			vec![ITEM_ID_0]
		);
	});
}

#[test]
fn destroy_trait_should_work() {
	ExtBuilder.build().execute_with(|| {
		let metadata: BoundedVec<u8, <Test as pallet_uniques::Config>::StringLimit> =
			b"metadata".to_vec().try_into().unwrap();

		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_0,
			CollectionType::Marketplace,
			metadata.clone()
		));

		assert_ok!(NFTPallet::do_mint(ALICE, COLLECTION_ID_0, ITEM_ID_0, metadata.clone()));

		let witness =
			<NFTPallet as Destroy<<Test as frame_system::Config>::AccountId>>::get_destroy_witness(&COLLECTION_ID_0)
				.unwrap();

		assert_eq!(
			witness,
			DestroyWitness {
				items: 1,
				item_metadatas: 0,
				attributes: 0
			}
		);

		// collection is not empty
		assert_noop!(
			<NFTPallet as Destroy<<Test as frame_system::Config>::AccountId>>::destroy(
				COLLECTION_ID_0,
				witness,
				Some(ALICE)
			),
			Error::<Test>::TokenCollectionNotEmpty
		);

		assert_ok!(<NFTPallet as Mutate<<Test as frame_system::Config>::AccountId>>::burn(
			&COLLECTION_ID_0,
			&ITEM_ID_0,
			None
		));

		let witness =
			<NFTPallet as Destroy<<Test as frame_system::Config>::AccountId>>::get_destroy_witness(&COLLECTION_ID_0)
				.unwrap();

		let empty_witness = DestroyWitness {
			items: 0,
			item_metadatas: 0,
			attributes: 0,
		};

		// we expect empty `witness`
		assert_eq!(witness, empty_witness);

		// not owner
		assert_noop!(
			<NFTPallet as Destroy<<Test as frame_system::Config>::AccountId>>::destroy(
				COLLECTION_ID_0,
				empty_witness,
				Some(BOB)
			),
			pallet_uniques::Error::<Test>::NoPermission
		);

		// with owner check
		assert_ok!(
			<NFTPallet as Destroy<<Test as frame_system::Config>::AccountId>>::destroy(
				COLLECTION_ID_0,
				witness,
				Some(ALICE)
			),
			witness
		);

		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_0,
			CollectionType::Marketplace,
			metadata,
		));

		// no owner check
		assert_ok!(
			<NFTPallet as Destroy<<Test as frame_system::Config>::AccountId>>::destroy(
				COLLECTION_ID_0,
				empty_witness,
				None
			),
			empty_witness
		);
	});
}

#[test]
fn mutate_trait_should_work() {
	ExtBuilder.build().execute_with(|| {
		let metadata: BoundedVec<u8, <Test as pallet_uniques::Config>::StringLimit> =
			b"metadata".to_vec().try_into().unwrap();

		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_0,
			CollectionType::Marketplace,
			metadata
		));

		// collection does not exist
		assert_noop!(
			<NFTPallet as Mutate<<Test as frame_system::Config>::AccountId>>::mint_into(
				&COLLECTION_ID_2,
				&ITEM_ID_0,
				&BOB
			),
			Error::<Test>::CollectionUnknown
		);

		assert_ok!(
			<NFTPallet as Mutate<<Test as frame_system::Config>::AccountId>>::mint_into(
				&COLLECTION_ID_0,
				&ITEM_ID_0,
				&ALICE
			)
		);

		// not owner
		assert_ok!(
			<NFTPallet as Mutate<<Test as frame_system::Config>::AccountId>>::mint_into(
				&COLLECTION_ID_0,
				&ITEM_ID_1,
				&BOB
			)
		);

		// not owner
		assert_noop!(
			<NFTPallet as Mutate<<Test as frame_system::Config>::AccountId>>::burn(
				&COLLECTION_ID_0,
				&ITEM_ID_0,
				Some(&BOB)
			),
			Error::<Test>::NotPermitted
		);

		// no owner check
		assert_ok!(<NFTPallet as Mutate<<Test as frame_system::Config>::AccountId>>::burn(
			&COLLECTION_ID_0,
			&ITEM_ID_1,
			None
		));
		assert!(!<Items<Test>>::contains_key(COLLECTION_ID_0, ITEM_ID_1));

		// with owner check
		assert_ok!(<NFTPallet as Mutate<<Test as frame_system::Config>::AccountId>>::burn(
			&COLLECTION_ID_0,
			&ITEM_ID_0,
			Some(&ALICE)
		));
		assert!(!<Items<Test>>::contains_key(COLLECTION_ID_0, ITEM_ID_0));

		// item does not exist
		assert_noop!(
			<NFTPallet as Mutate<<Test as frame_system::Config>::AccountId>>::burn(
				&COLLECTION_ID_0,
				&ITEM_ID_0,
				Some(&ALICE)
			),
			Error::<Test>::ItemUnknown
		);
	});
}

#[test]
fn transfer_trait_should_work() {
	ExtBuilder.build().execute_with(|| {
		// collection does not exist
		assert_noop!(
			<NFTPallet as Transfer<<Test as frame_system::Config>::AccountId>>::transfer(
				&COLLECTION_ID_1,
				&ITEM_ID_0,
				&ALICE
			),
			Error::<Test>::ItemUnknown
		);

		// item does not exist
		assert_noop!(
			<NFTPallet as Transfer<<Test as frame_system::Config>::AccountId>>::transfer(
				&COLLECTION_ID_0,
				&ITEM_ID_1,
				&ALICE
			),
			Error::<Test>::ItemUnknown
		);

		let metadata: BoundedVec<u8, <Test as pallet_uniques::Config>::StringLimit> =
			b"metadata".to_vec().try_into().unwrap();

		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_0,
			CollectionType::Marketplace,
			metadata.clone()
		));

		assert_ok!(NFTPallet::do_mint(ALICE, COLLECTION_ID_0, ITEM_ID_0, metadata));

		assert_ok!(
			<NFTPallet as Transfer<<Test as frame_system::Config>::AccountId>>::transfer(
				&COLLECTION_ID_0,
				&ITEM_ID_0,
				&ALICE
			)
		);
		assert_eq!(NFTPallet::owner(&COLLECTION_ID_0, &ITEM_ID_0), Some(ALICE));
	});
}

#[test]
fn is_id_reserved_should_return_true_when_id_is_from_reserved_range() {
	assert!(
		NFTPallet::is_id_reserved(0),
		"0 should be part of reserved CollectionId range"
	);

	assert!(
		NFTPallet::is_id_reserved(13),
		"num <= ReserveCollectionIdUpTo should be part of reserved CollectionId range"
	);

	assert!(
		NFTPallet::is_id_reserved(mock::ReserveCollectionIdUpTo::get()),
		"num == ReserveCollectionIdUpTo should be part of reserved CollectionId range"
	);
}

#[test]
fn is_id_reserved_should_return_false_when_id_is_not_from_reserved_range() {
	assert!(
		!NFTPallet::is_id_reserved(mock::ReserveCollectionIdUpTo::get() + 1),
		"(ReserveCollectionIdUpTo + 1) should not be part of reserved CollectionId range"
	);

	assert!(
		!NFTPallet::is_id_reserved(mock::ReserveCollectionIdUpTo::get() + 500_000_000_000),
		"num > ReserveCollectionIdUpTo should not be part of reserved CollectionId range"
	);
}

#[test]
fn create_typed_collection_should_work_without_deposit_when_deposit_is_not_required() {
	ExtBuilder.build().execute_with(|| {
		let metadata: BoundedVec<u8, <Test as pallet_uniques::Config>::StringLimit> =
			b"metadata".to_vec().try_into().unwrap();

		assert_ok!(NFTPallet::create_typed_collection(
			ACCOUNT_WITH_NO_BALANCE,
			COLLECTION_ID_0,
			CollectionType::LiquidityMining,
			Some(metadata.clone()),
		));

		assert_eq!(
			NFTPallet::collections(COLLECTION_ID_0).unwrap(),
			CollectionInfoOf::<Test> {
				collection_type: CollectionType::LiquidityMining,
				metadata
			}
		)
	});
}

#[test]
fn create_typed_collection_should_work_with_reserved_id() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(NFTPallet::create_typed_collection(
			ALICE,
			COLLECTION_ID_RESERVED,
			CollectionType::LiquidityMining,
			None,
		));

		assert_eq!(
			NFTPallet::collections(COLLECTION_ID_RESERVED).unwrap(),
			CollectionInfoOf::<Test> {
				collection_type: CollectionType::LiquidityMining,
				metadata: Default::default()
			}
		)
	});
}

#[test]
fn create_typed_collection_should_not_work_without_deposit_when_deposit_is_required() {
	ExtBuilder.build().execute_with(|| {
		assert_noop!(
			NFTPallet::create_typed_collection(
				ACCOUNT_WITH_NO_BALANCE,
				COLLECTION_ID_0,
				CollectionType::Marketplace,
				None,
			),
			pallet_balances::Error::<Test>::InsufficientBalance
		);
	});
}

#[test]
fn do_mint_should_work_when_account_has_no_balance() {
	ExtBuilder.build().execute_with(|| {
		//arrange
		assert_ok!(NFTPallet::create_typed_collection(
			ACCOUNT_WITH_NO_BALANCE,
			COLLECTION_ID_0,
			CollectionType::LiquidityMining,
			None,
		));

		//act & assert
		assert_ok!(NFTPallet::mint_into(
			&COLLECTION_ID_0,
			&ITEM_ID_0,
			&ACCOUNT_WITH_NO_BALANCE,
		));
	});
}

#[test]
fn burn_should_work_when_account_has_no_balance() {
	ExtBuilder.build().execute_with(|| {
		//arrange
		assert_ok!(NFTPallet::create_typed_collection(
			ACCOUNT_WITH_NO_BALANCE,
			COLLECTION_ID_0,
			CollectionType::LiquidityMining,
			None,
		));

		assert_ok!(NFTPallet::mint_into(
			&COLLECTION_ID_0,
			&ITEM_ID_0,
			&ACCOUNT_WITH_NO_BALANCE,
		));
		assert_ok!(NFTPallet::mint_into(
			&COLLECTION_ID_0,
			&ITEM_ID_1,
			&ACCOUNT_WITH_NO_BALANCE,
		));

		//act & assert
		assert_noop!(
			<NFTPallet as Mutate<<Test as frame_system::Config>::AccountId>>::burn(
				&COLLECTION_ID_0,
				&ITEM_ID_0,
				Some(&ALICE) // not owner
			),
			Error::<Test>::NotPermitted
		);

		assert_ok!(<NFTPallet as Mutate<<Test as frame_system::Config>::AccountId>>::burn(
			&COLLECTION_ID_0,
			&ITEM_ID_0,
			None
		));
		assert_ok!(<NFTPallet as Mutate<<Test as frame_system::Config>::AccountId>>::burn(
			&COLLECTION_ID_0,
			&ITEM_ID_1,
			Some(&ACCOUNT_WITH_NO_BALANCE)
		));
	});
}

#[test]
fn do_destroy_collection_works() {
	ExtBuilder.build().execute_with(|| {
		let metadata: BoundedVec<u8, <Test as pallet_uniques::Config>::StringLimit> =
			b"metadata".to_vec().try_into().unwrap();

		// collection does not exist
		assert_noop!(
			NFTPallet::do_destroy_collection(ALICE, COLLECTION_ID_0),
			Error::<Test>::CollectionUnknown
		);

		// existing item
		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_0,
			CollectionType::Marketplace,
			metadata.clone()
		));

		assert_ok!(NFTPallet::do_mint(ALICE, COLLECTION_ID_0, ITEM_ID_0, metadata.clone()));

		assert_noop!(
			NFTPallet::do_destroy_collection(ALICE, COLLECTION_ID_0),
			Error::<Test>::TokenCollectionNotEmpty
		);

		// happy path
		assert_ok!(NFTPallet::do_burn(ALICE, COLLECTION_ID_0, ITEM_ID_0));

		let witness = NFTPallet::do_destroy_collection(ALICE, COLLECTION_ID_0).unwrap();
		assert_eq!(
			witness,
			DestroyWitness {
				items: 0,
				item_metadatas: 0,
				attributes: 0
			}
		);

		assert_eq!(NFTPallet::collections(COLLECTION_ID_0), None);

		expect_events(vec![crate::Event::CollectionDestroyed {
			owner: ALICE,
			collection_id: COLLECTION_ID_0,
		}
		.into()]);

		// permissions are ignored
		assert_ok!(NFTPallet::do_create_collection(
			ALICE,
			COLLECTION_ID_1,
			CollectionType::LiquidityMining,
			metadata
		));

		assert_ok!(NFTPallet::do_destroy_collection(ALICE, COLLECTION_ID_1));
	});
}
