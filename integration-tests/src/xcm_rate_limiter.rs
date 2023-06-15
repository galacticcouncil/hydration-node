#![cfg(test)]

use crate::polkadot_test_net::*;

use common_runtime::Weight;
use frame_support::assert_ok;
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
	*message_hash
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
			WeightLimit::Limited(Weight::from_ref_time(399_600_000_000))
		));

		message_hash = get_message_hash_from_event(2);

		// Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(&AccountId::from(ALICE)),
			100 * UNITS
		);
	});

	Hydra::execute_with(|| {
		expect_hydra_events(vec![
			cumulus_pallet_xcmp_queue::Event::XcmDeferred {
				sender: ACALA_PARA_ID.into(),
				sent_at: 3,
				deferred_to: 604, // received at 4 plus 600 blocks of deferral
				message_hash,
			}
			.into(),
			pallet_relaychain_info::Event::CurrentBlockNumbers {
				parachain_block_number: 1,
				relaychain_block_number: 5,
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
			WeightLimit::Limited(Weight::from_ref_time(399_600_000_000))
		));

		// Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(&AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE_ON_OTHER_PARACHAIN - amount
		);
	});

	let fee = 400641025641;
	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(ACA, &AccountId::from(BOB)),
			amount - fee
		);
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(ACA, &hydradx_runtime::Treasury::account_id()),
			fee // fees should go to treasury
		);
	});
}
