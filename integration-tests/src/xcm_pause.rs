#![cfg(test)]

use crate::polkadot_test_net::*;
use cumulus_primitives_core::AggregateMessageOrigin;
use frame_support::dispatch::GetDispatchInfo;
use frame_support::traits::{QueueFootprintQuery, ServiceQueues};
use frame_support::weights::Weight;
use frame_support::{assert_noop, assert_ok};
use hydradx_runtime::{LocationToAccountId, RuntimeOrigin, TechnicalCollective};
use orml_traits::MultiCurrency;
use polkadot_xcm::v5::prelude::*;
use polkadot_xcm::{VersionedAssets, VersionedXcm};
use pretty_assertions::assert_eq;
use primitives::AccountId;
use sp_runtime::codec::Encode;
use sp_runtime::DispatchError::BadOrigin;
use xcm_emulator::{ConvertLocation, TestExt};

/// Any sibling parachain will do; the emulator's second parachain stands in for it.
type Sibling = Acala;
const SIBLING_PARA_ID: u32 = ACALA_PARA_ID;
const SIBLING_ASSET: primitives::AssetId = ACA;

fn tech_committee_supermajority() -> RuntimeOrigin {
	pallet_collective::RawOrigin::<AccountId, TechnicalCollective>::Members(3, 4).into()
}

fn make_bob_the_only_tech_committee_member() {
	pallet_collective::Members::<hydradx_runtime::Runtime, TechnicalCollective>::put(vec![AccountId::from(BOB)]);
}

fn suspend_xcm_send() {
	assert_ok!(hydradx_runtime::Parameters::suspend_xcm_send(RuntimeOrigin::root()));
}

fn transport_error_at(index: u8) -> pallet_xcm::Error<hydradx_runtime::Runtime> {
	pallet_xcm::Error::<hydradx_runtime::Runtime>::LocalExecutionIncompleteWithError {
		index,
		error: pallet_xcm::ExecutionError::Transport,
	}
}

fn sibling() -> Location {
	Location::new(1, [Parachain(SIBLING_PARA_ID)])
}

fn hydra() -> Location {
	Location::new(1, [Parachain(HYDRA_PARA_ID)])
}

fn native_of(para_id: u32, amount: u128) -> Asset {
	Asset {
		id: AssetId(Location::new(1, [Parachain(para_id), GeneralIndex(0)])),
		fun: Fungible(amount),
	}
}

fn bob() -> Location {
	Location::new(0, [AccountId32 { id: BOB, network: None }])
}

fn register_dot_location_on_hydra() {
	assert_ok!(hydradx_runtime::AssetRegistry::set_location(
		1,
		hydradx_runtime::AssetLocation(Location {
			parents: 1,
			interior: Here
		})
	));
}

fn send_dot_from_hydra_to_bob_on_rococo() -> sp_runtime::DispatchResult {
	hydradx_runtime::XTokens::transfer(
		RuntimeOrigin::signed(ALICE.into()),
		1,
		3 * UNITS,
		Box::new(
			Location {
				parents: 1,
				interior: [Junction::AccountId32 { id: BOB, network: None }].into(),
			}
			.into_versioned(),
		),
		WeightLimit::Unlimited,
	)
}

/// Pays for execution on the sibling from Hydra's sovereign account there, then transfers `UNITS` to BOB.
fn transact_transfer_to_bob_on_sibling() -> Xcm<()> {
	let call = pallet_balances::Call::<hydradx_runtime::Runtime>::transfer_allow_death {
		dest: BOB.into(),
		value: UNITS,
	};
	Xcm(vec![
		WithdrawAsset(native_of(SIBLING_PARA_ID, 900 * UNITS).into()),
		BuyExecution {
			fees: native_of(SIBLING_PARA_ID, 800 * UNITS),
			weight_limit: Unlimited,
		},
		Transact {
			fallback_max_weight: Some(call.get_dispatch_info().call_weight),
			origin_kind: OriginKind::SovereignAccount,
			call: hydradx_runtime::RuntimeCall::Balances(call).encode().into(),
		},
		ExpectTransactStatus(MaybeErrorCode::Success),
		RefundSurplus,
		DepositAsset {
			assets: All.into(),
			beneficiary: hydra(),
		},
	])
}

fn send_transact_from_hydra_to_sibling() -> sp_runtime::DispatchResult {
	hydradx_runtime::PolkadotXcm::send(
		RuntimeOrigin::root(),
		Box::new(sibling().into_versioned()),
		Box::new(VersionedXcm::from(transact_transfer_to_bob_on_sibling())),
	)
}

fn fund_hydra_sovereign_on_sibling() {
	Sibling::execute_with(|| {
		let hydra_sovereign = LocationToAccountId::convert_location(&hydra()).unwrap();
		assert_ok!(hydradx_runtime::Balances::transfer_allow_death(
			RuntimeOrigin::signed(ALICE.into()),
			hydra_sovereign,
			1_000 * UNITS,
		));
	});
}

fn execute_reserve_transfer_of_hdx_to_sibling() -> frame_support::dispatch::DispatchResultWithPostInfo {
	let message = Xcm::<hydradx_runtime::RuntimeCall>(vec![
		WithdrawAsset(native_of(HYDRA_PARA_ID, 10 * UNITS).into()),
		DepositReserveAsset {
			assets: All.into(),
			dest: sibling(),
			xcm: Xcm(vec![
				BuyExecution {
					fees: native_of(HYDRA_PARA_ID, 10 * UNITS),
					weight_limit: Unlimited,
				},
				DepositAsset {
					assets: All.into(),
					beneficiary: bob(),
				},
			]),
		},
	]);
	hydradx_runtime::PolkadotXcm::execute(
		RuntimeOrigin::signed(ALICE.into()),
		Box::new(VersionedXcm::from(message)),
		Weight::from_parts(399_600_000_000, 0),
	)
}

fn transfer_assets_of_hdx_to_sibling() -> sp_runtime::DispatchResult {
	hydradx_runtime::PolkadotXcm::transfer_assets(
		RuntimeOrigin::signed(ALICE.into()),
		Box::new(sibling().into_versioned()),
		Box::new(bob().into_versioned()),
		Box::new(VersionedAssets::from(Assets::from(native_of(
			HYDRA_PARA_ID,
			10 * UNITS,
		)))),
		0,
		WeightLimit::Unlimited,
	)
}

fn send_sibling_asset_to_bob_on_hydra() {
	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			SIBLING_ASSET,
			hydradx_runtime::AssetLocation(Location {
				parents: 1,
				interior: [Junction::Parachain(SIBLING_PARA_ID), Junction::GeneralIndex(0)].into()
			})
		));
	});

	Sibling::execute_with(|| {
		assert_ok!(hydradx_runtime::XTokens::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			0,
			30 * UNITS,
			Box::new(
				Location {
					parents: 1,
					interior: [
						Junction::Parachain(HYDRA_PARA_ID),
						Junction::AccountId32 { id: BOB, network: None }
					]
					.into()
				}
				.into_versioned()
			),
			WeightLimit::Limited(Weight::from_parts(399_600_000_000, 0))
		));
	});
}

fn sibling_queue_message_count() -> u64 {
	hydradx_runtime::MessageQueue::footprint(AggregateMessageOrigin::Sibling(SIBLING_PARA_ID.into()))
		.storage
		.count
}

#[test]
fn xtokens_transfer_should_fail_when_xcm_send_is_suspended() {
	TestNet::reset();

	Hydra::execute_with(|| {
		register_dot_location_on_hydra();
		suspend_xcm_send();

		assert_noop!(
			send_dot_from_hydra_to_bob_on_rococo(),
			orml_xtokens::Error::<hydradx_runtime::Runtime>::XcmExecutionFailed
		);
	});
}

#[test]
fn xtokens_transfer_should_reach_rococo_when_xcm_send_is_resumed() {
	TestNet::reset();

	Hydra::execute_with(|| {
		register_dot_location_on_hydra();
		suspend_xcm_send();
		assert_ok!(hydradx_runtime::Parameters::resume_xcm_send(RuntimeOrigin::root()));

		assert_ok!(send_dot_from_hydra_to_bob_on_rococo());
	});

	Rococo::execute_with(|| {
		assert!(hydradx_runtime::Balances::free_balance(AccountId::from(BOB)) > 0);
	});
}

#[test]
fn polkadot_xcm_send_to_relay_should_fail_when_xcm_send_is_suspended() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let send_to_relay = || {
			hydradx_runtime::PolkadotXcm::send(
				RuntimeOrigin::root(),
				Box::new(Location::parent().into_versioned()),
				Box::new(VersionedXcm::from(Xcm::<()>(vec![ClearOrigin]))),
			)
		};
		assert_ok!(send_to_relay());
		suspend_xcm_send();

		assert_noop!(
			send_to_relay(),
			pallet_xcm::Error::<hydradx_runtime::Runtime>::SendFailure
		);
	});
}

#[test]
fn polkadot_xcm_send_transact_to_sibling_should_fail_when_xcm_send_is_suspended() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(send_transact_from_hydra_to_sibling());
		suspend_xcm_send();

		assert_noop!(
			send_transact_from_hydra_to_sibling(),
			pallet_xcm::Error::<hydradx_runtime::Runtime>::SendFailure
		);
	});
}

#[test]
fn polkadot_xcm_send_transact_should_execute_on_sibling_when_xcm_send_is_resumed() {
	TestNet::reset();
	fund_hydra_sovereign_on_sibling();
	let bob_balance_before = Sibling::execute_with(|| hydradx_runtime::Balances::free_balance(AccountId::from(BOB)));

	Hydra::execute_with(|| {
		suspend_xcm_send();
		assert_ok!(hydradx_runtime::Parameters::resume_xcm_send(RuntimeOrigin::root()));

		assert_ok!(send_transact_from_hydra_to_sibling());
	});

	Sibling::execute_with(|| {
		assert_xcm_message_processing_passed();
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(BOB)),
			bob_balance_before + UNITS
		);
	});
}

#[test]
fn polkadot_xcm_execute_should_fail_when_xcm_send_is_suspended() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(execute_reserve_transfer_of_hdx_to_sibling());
		suspend_xcm_send();

		assert_noop!(
			execute_reserve_transfer_of_hdx_to_sibling().map_err(|e| e.error),
			transport_error_at(1)
		);
	});
}

#[test]
fn polkadot_xcm_transfer_assets_should_fail_when_xcm_send_is_suspended() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(transfer_assets_of_hdx_to_sibling());
		suspend_xcm_send();

		assert_noop!(transfer_assets_of_hdx_to_sibling(), transport_error_at(2));
	});
}

fn send_hdx_round_trip_from_sibling_via_hydra() {
	Sibling::execute_with(|| {
		let message = Xcm(vec![
			WithdrawAsset(native_of(HYDRA_PARA_ID, 900 * UNITS).into()),
			BuyExecution {
				fees: native_of(HYDRA_PARA_ID, 800 * UNITS),
				weight_limit: Unlimited,
			},
			DepositReserveAsset {
				assets: All.into(),
				dest: sibling(),
				xcm: Xcm(vec![DepositAsset {
					assets: All.into(),
					beneficiary: bob(),
				}]),
			},
		]);
		assert_ok!(hydradx_runtime::PolkadotXcm::send_xcm(Here, hydra(), message));
	});
}

fn fund_sibling_sovereign_on_hydra() {
	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::Balances::transfer_allow_death(
			RuntimeOrigin::signed(ALICE.into()),
			parachain_reserve_account(),
			1_000 * UNITS,
		));
	});
}

fn assets_trapped_on_hydra() -> bool {
	hydradx_runtime::System::events().iter().any(|r| {
		matches!(
			r.event,
			hydradx_runtime::RuntimeEvent::PolkadotXcm(pallet_xcm::Event::AssetsTrapped { .. })
		)
	})
}

#[test]
fn inbound_xcmp_message_should_forward_onward_when_xcm_send_is_not_suspended() {
	TestNet::reset();
	fund_sibling_sovereign_on_hydra();

	send_hdx_round_trip_from_sibling_via_hydra();

	Hydra::execute_with(|| {
		assert_xcm_message_processing_passed();
		assert!(!assets_trapped_on_hydra());
	});
}

#[test]
fn inbound_xcmp_message_should_trap_assets_when_it_sends_onward_while_xcm_send_is_suspended() {
	TestNet::reset();
	fund_sibling_sovereign_on_hydra();
	Hydra::execute_with(suspend_xcm_send);

	send_hdx_round_trip_from_sibling_via_hydra();

	Hydra::execute_with(|| {
		assert_xcm_message_processing_failed();
		assert!(assets_trapped_on_hydra());
	});
}

#[test]
fn suspend_xcm_send_should_succeed_when_origin_is_tech_committee_supermajority() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::Parameters::suspend_xcm_send(
			tech_committee_supermajority()
		));

		assert!(hydradx_runtime::Parameters::xcm_send_suspended());
	});
}

#[test]
fn inbound_xcmp_transfer_should_stay_queued_when_xcm_execution_is_suspended() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::Parameters::suspend_xcm_execution(RuntimeOrigin::root()));
	});

	send_sibling_asset_to_bob_on_hydra();

	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(SIBLING_ASSET, &AccountId::from(BOB)),
			0
		);
		assert_eq!(sibling_queue_message_count(), 1);
	});
}

#[test]
fn inbound_xcmp_transfer_should_execute_when_xcm_execution_is_resumed() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::Parameters::suspend_xcm_execution(RuntimeOrigin::root()));
	});

	send_sibling_asset_to_bob_on_hydra();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::Parameters::resume_xcm_execution(RuntimeOrigin::root()));
		hydradx_runtime::MessageQueue::service_queues(Weight::MAX);

		assert_eq!(sibling_queue_message_count(), 0);
		let fee = hydradx_runtime::Tokens::free_balance(SIBLING_ASSET, &hydradx_runtime::Treasury::account_id());
		assert!(fee > 0, "Fee is not sent to treasury");
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(SIBLING_ASSET, &AccountId::from(BOB)),
			30 * UNITS - fee
		);
	});
}

#[test]
fn inbound_xcmp_transact_should_not_execute_when_xcm_execution_is_suspended() {
	TestNet::reset();
	fund_hydra_sovereign_on_sibling();
	let bob_balance_before = Sibling::execute_with(|| hydradx_runtime::Balances::free_balance(AccountId::from(BOB)));

	Sibling::execute_with(|| {
		assert_ok!(hydradx_runtime::Parameters::suspend_xcm_execution(RuntimeOrigin::root()));
	});

	Hydra::execute_with(|| {
		assert_ok!(send_transact_from_hydra_to_sibling());
	});

	Sibling::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(BOB)),
			bob_balance_before
		);
		assert_eq!(
			hydradx_runtime::MessageQueue::footprint(AggregateMessageOrigin::Sibling(HYDRA_PARA_ID.into()))
				.storage
				.count,
			1
		);
	});
}

/// Pays for execution from the relay chain's account on Hydra, then deposits everything to BOB.
fn send_hdx_from_relay_account_to_bob_on_hydra() {
	Hydra::execute_with(|| {
		let relay_account = LocationToAccountId::convert_location(&Location::parent()).unwrap();
		assert_ok!(hydradx_runtime::Balances::transfer_allow_death(
			RuntimeOrigin::signed(ALICE.into()),
			relay_account,
			1_000 * UNITS,
		));
	});

	Rococo::execute_with(|| {
		// The test relay has no head for Hydra, and DMP refuses to route to unknown paras.
		polkadot_runtime_parachains::paras::Heads::<rococo_runtime::Runtime>::insert(
			cumulus_primitives_core::ParaId::from(HYDRA_PARA_ID),
			polkadot_parachain::primitives::HeadData(vec![]),
		);
		let message = Xcm(vec![
			WithdrawAsset(native_of(HYDRA_PARA_ID, 900 * UNITS).into()),
			BuyExecution {
				fees: native_of(HYDRA_PARA_ID, 800 * UNITS),
				weight_limit: Unlimited,
			},
			DepositAsset {
				assets: All.into(),
				beneficiary: bob(),
			},
		]);
		assert_ok!(rococo_runtime::XcmPallet::send_xcm(
			Here,
			Location::new(0, [Parachain(HYDRA_PARA_ID)]),
			message
		));
	});
}

fn relay_queue_message_count() -> u64 {
	hydradx_runtime::MessageQueue::footprint(AggregateMessageOrigin::Parent)
		.storage
		.count
}

#[test]
fn inbound_dmp_message_should_stay_queued_when_xcm_execution_is_suspended() {
	TestNet::reset();
	let bob_balance_before = Hydra::execute_with(|| hydradx_runtime::Balances::free_balance(AccountId::from(BOB)));
	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::Parameters::suspend_xcm_execution(RuntimeOrigin::root()));
	});

	send_hdx_from_relay_account_to_bob_on_hydra();

	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(BOB)),
			bob_balance_before
		);
		assert_eq!(relay_queue_message_count(), 1);
	});
}

#[test]
fn inbound_dmp_message_should_execute_when_xcm_execution_is_resumed() {
	TestNet::reset();
	let bob_balance_before = Hydra::execute_with(|| hydradx_runtime::Balances::free_balance(AccountId::from(BOB)));
	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::Parameters::suspend_xcm_execution(RuntimeOrigin::root()));
	});

	send_hdx_from_relay_account_to_bob_on_hydra();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::Parameters::resume_xcm_execution(RuntimeOrigin::root()));
		hydradx_runtime::MessageQueue::service_queues(Weight::MAX);

		assert_xcm_message_processing_passed();
		assert_eq!(relay_queue_message_count(), 0);
		assert!(hydradx_runtime::Balances::free_balance(AccountId::from(BOB)) > bob_balance_before);
	});
}

#[test]
fn suspend_xcm_execution_should_succeed_when_origin_is_tech_committee_supermajority() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::Parameters::suspend_xcm_execution(
			tech_committee_supermajority()
		));
	});
}

#[test]
fn suspend_xcm_execution_should_fail_when_origin_is_single_tech_committee_member() {
	TestNet::reset();

	Hydra::execute_with(|| {
		make_bob_the_only_tech_committee_member();

		assert_noop!(
			hydradx_runtime::Parameters::suspend_xcm_execution(RuntimeOrigin::signed(BOB.into())),
			BadOrigin
		);
	});
}

#[test]
fn suspend_xcm_should_block_outbound_xcm_when_called_by_single_tech_committee_member() {
	TestNet::reset();

	Hydra::execute_with(|| {
		register_dot_location_on_hydra();
		make_bob_the_only_tech_committee_member();

		assert_ok!(hydradx_runtime::Parameters::suspend_xcm(RuntimeOrigin::signed(
			BOB.into()
		)));

		assert_noop!(
			send_dot_from_hydra_to_bob_on_rococo(),
			orml_xtokens::Error::<hydradx_runtime::Runtime>::XcmExecutionFailed
		);
	});
}

#[test]
fn suspend_xcm_should_hold_inbound_xcm_when_called_by_single_tech_committee_member() {
	TestNet::reset();
	Hydra::execute_with(|| {
		make_bob_the_only_tech_committee_member();
		assert_ok!(hydradx_runtime::Parameters::suspend_xcm(RuntimeOrigin::signed(
			BOB.into()
		)));
	});

	send_sibling_asset_to_bob_on_hydra();

	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(SIBLING_ASSET, &AccountId::from(BOB)),
			0
		);
		assert_eq!(sibling_queue_message_count(), 1);
	});
}

#[test]
fn suspend_xcm_should_fail_when_caller_is_not_tech_committee_member() {
	TestNet::reset();

	Hydra::execute_with(|| {
		make_bob_the_only_tech_committee_member();

		assert_noop!(
			hydradx_runtime::Parameters::suspend_xcm(RuntimeOrigin::signed(ALICE.into())),
			BadOrigin
		);
	});
}

#[test]
fn resume_xcm_should_fail_when_called_by_single_tech_committee_member() {
	TestNet::reset();

	Hydra::execute_with(|| {
		make_bob_the_only_tech_committee_member();
		assert_ok!(hydradx_runtime::Parameters::suspend_xcm(RuntimeOrigin::signed(
			BOB.into()
		)));

		assert_noop!(
			hydradx_runtime::Parameters::resume_xcm(RuntimeOrigin::signed(BOB.into())),
			BadOrigin
		);
	});
}

#[test]
fn resume_xcm_should_restore_outbound_xcm_when_origin_is_tech_committee_supermajority() {
	TestNet::reset();

	Hydra::execute_with(|| {
		register_dot_location_on_hydra();
		make_bob_the_only_tech_committee_member();
		assert_ok!(hydradx_runtime::Parameters::suspend_xcm(RuntimeOrigin::signed(
			BOB.into()
		)));

		assert_ok!(hydradx_runtime::Parameters::resume_xcm(tech_committee_supermajority()));

		assert_ok!(send_dot_from_hydra_to_bob_on_rococo());
	});
}
