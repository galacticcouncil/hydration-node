#![cfg(test)]

use crate::polkadot_test_net::*;

use frame_support::{assert_ok, pallet_prelude::Weight};
use orml_traits::currency::MultiCurrency;
use pallet_asset_registry::AssetType;
use polkadot_xcm::prelude::*;
use xcm_emulator::TestExt;

/// Returns the message hash in the `XcmpMessageSent` event at the `n`th last event (1-indexed, so if the second to last
/// event has the hash, pass `2`);
fn get_message_hash_from_event(n: usize) -> Option<[u8; 32]> {
	use cumulus_pallet_xcmp_queue::Event;
	use hydradx_runtime::RuntimeEvent;
	let RuntimeEvent::XcmpQueue(Event::XcmpMessageSent { message_hash }) = &last_hydra_events(n)[0] else {
		panic!("expecting to find message sent event");
	};
	Some(*message_hash)
}

#[test]
fn xcm_rate_limiter_should_limit_aca_when_limit_is_exceeded() {
	// Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			hydradx_runtime::RuntimeOrigin::root(),
			ACA,
			hydradx_runtime::AssetLocation(MultiLocation::new(1, X2(Parachain(ACALA_PARA_ID), GeneralIndex(0))))
		));

		// set an xcm rate limit
		assert_ok!(hydradx_runtime::AssetRegistry::update(
			hydradx_runtime::RuntimeOrigin::root(),
			ACA,
			b"ACA".to_vec(),
			AssetType::Token,
			None,
			Some(50 * UNITS),
		));

		assert_eq!(hydradx_runtime::Tokens::free_balance(ACA, &AccountId::from(BOB)), 0);

		//Set it to same as the relay block number should be in XcmDeferFilter
		//since we use different RelayChainBlockNumberProvider in runtime-benchmark feature
		//where we return frame_system current time
		frame_system::Pallet::<hydradx_runtime::Runtime>::set_block_number(4);
	});

	let amount = 100 * UNITS;
	let mut message_hash = None;
	Acala::execute_with(|| {
		// Act
		assert_ok!(hydradx_runtime::XTokens::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			0,
			amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Junction::Parachain(HYDRA_PARA_ID),
						Junction::AccountId32 { id: BOB, network: None }
					)
				)
				.into()
			),
			WeightLimit::Limited(Weight::from_parts(399_600_000_000, 0))
		));

		message_hash = get_message_hash_from_event(2);

		// Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - amount
		);
	});

	let relay_block = PolkadotRelay::execute_with(polkadot_runtime::System::block_number);

	Hydra::execute_with(|| {
		expect_hydra_events(vec![
			cumulus_pallet_xcmp_queue::Event::XcmDeferred {
				sender: ACALA_PARA_ID.into(),
				sent_at: relay_block,
				deferred_to: hydradx_runtime::DeferDuration::get() + relay_block - 1,
				message_hash,
				index: (hydradx_runtime::DeferDuration::get() + relay_block - 1, 0),
				position: 0,
			}
			.into(),
			pallet_relaychain_info::Event::CurrentBlockNumbers {
				parachain_block_number: 5,
				relaychain_block_number: relay_block + 1,
			}
			.into(),
		]);
		assert_eq!(hydradx_runtime::Tokens::free_balance(ACA, &AccountId::from(BOB)), 0);
	});
}

#[test]
fn xcm_rate_limiter_should_not_limit_aca_when_limit_is_not_exceeded() {
	// Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			hydradx_runtime::RuntimeOrigin::root(),
			ACA,
			hydradx_runtime::AssetLocation(MultiLocation::new(1, X2(Parachain(ACALA_PARA_ID), GeneralIndex(0))))
		));

		// set an xcm rate limit
		assert_ok!(hydradx_runtime::AssetRegistry::update(
			hydradx_runtime::RuntimeOrigin::root(),
			ACA,
			b"ACA".to_vec(),
			AssetType::Token,
			None,
			Some(101 * UNITS),
		));
	});

	let amount = 100 * UNITS;
	Acala::execute_with(|| {
		// Act
		assert_ok!(hydradx_runtime::XTokens::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			0,
			amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Junction::Parachain(HYDRA_PARA_ID),
						Junction::AccountId32 { id: BOB, network: None }
					)
				)
				.into()
			),
			WeightLimit::Limited(Weight::from_parts(399_600_000_000, 0))
		));

		// Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - amount
		);
	});

	Hydra::execute_with(|| {
		let fee = hydradx_runtime::Tokens::free_balance(ACA, &hydradx_runtime::Treasury::account_id());
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(ACA, &AccountId::from(BOB)),
			amount - fee
		);
	});
}

#[test]
fn deferred_messages_should_be_executable_by_root() {
	// Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			hydradx_runtime::RuntimeOrigin::root(),
			ACA,
			hydradx_runtime::AssetLocation(MultiLocation::new(1, X2(Parachain(ACALA_PARA_ID), GeneralIndex(0))))
		));

		// set an xcm rate limit
		assert_ok!(hydradx_runtime::AssetRegistry::update(
			hydradx_runtime::RuntimeOrigin::root(),
			ACA,
			b"ACA".to_vec(),
			AssetType::Token,
			None,
			Some(50 * UNITS),
		));

		assert_eq!(hydradx_runtime::Tokens::free_balance(ACA, &AccountId::from(BOB)), 0);

		//Set it to same as the relay block number should be in XcmDeferFilter
		//since we use different RelayChainBlockNumberProvider in runtime-benchmark feature
		//where we return frame_system current time
		frame_system::Pallet::<hydradx_runtime::Runtime>::set_block_number(4);
	});

	let amount = 100 * UNITS;
	let mut message_hash = None;
	let max_weight = Weight::from_parts(399_600_000_000, 0);
	Acala::execute_with(|| {
		// Act
		assert_ok!(hydradx_runtime::XTokens::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			0,
			amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Junction::Parachain(HYDRA_PARA_ID),
						Junction::AccountId32 { id: BOB, network: None }
					)
				)
				.into()
			),
			WeightLimit::Limited(max_weight),
		));

		message_hash = get_message_hash_from_event(2);

		// Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - amount
		);
	});

	let relay_block = PolkadotRelay::execute_with(polkadot_runtime::System::block_number);

	Hydra::execute_with(|| {
		expect_hydra_events(vec![
			cumulus_pallet_xcmp_queue::Event::XcmDeferred {
				sender: ACALA_PARA_ID.into(),
				sent_at: relay_block,
				deferred_to: hydradx_runtime::DeferDuration::get() + relay_block - 1,
				message_hash,
				index: (hydradx_runtime::DeferDuration::get() + relay_block - 1, 0),
				position: 0,
			}
			.into(),
			pallet_relaychain_info::Event::CurrentBlockNumbers {
				parachain_block_number: 5,
				relaychain_block_number: relay_block + 1,
			}
			.into(),
		]);
		assert_eq!(hydradx_runtime::Tokens::free_balance(ACA, &AccountId::from(BOB)), 0);

		set_relaychain_block_number(hydradx_runtime::DeferDuration::get() + relay_block);

		assert_eq!(hydradx_runtime::Tokens::free_balance(ACA, &AccountId::from(BOB)), 0);
		assert_ok!(hydradx_runtime::XcmpQueue::service_deferred(
			hydradx_runtime::RuntimeOrigin::root(),
			hydradx_runtime::ReservedXcmpWeight::get(),
			ACALA_PARA_ID.into(),
			hydradx_runtime::MaxDeferredBuckets::get(),
		));

		let fee = hydradx_runtime::Tokens::free_balance(ACA, &hydradx_runtime::Treasury::account_id());
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(ACA, &AccountId::from(BOB)),
			amount - fee
		);
	});
}
