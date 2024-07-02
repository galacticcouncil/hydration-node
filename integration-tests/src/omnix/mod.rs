mod intents;
mod solution;

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::traits::fungibles::Mutate;
use hydradx_runtime::{Currencies, OmniX, Runtime, RuntimeOrigin};
use pallet_omnix::types::{IntentId, Swap};
use primitives::{AccountId, AssetId, Moment};
use sp_runtime::DispatchResult;
use xcm_emulator::TestExt;

pub(crate) fn submit_intents(intents: Vec<(AccountId, Swap<AssetId>, Moment)>) -> Vec<IntentId> {
	let mut intent_ids = Vec::new();
	for (who, swap, deadline) in intents {
		let increment_id = pallet_omnix::Pallet::<hydradx_runtime::Runtime>::next_incremental_id();
		assert_ok!(OmniX::submit_intent(
			RuntimeOrigin::signed(who),
			swap,
			deadline,
			false,
			None,
			None,
		));
		let intent_id = pallet_omnix::Pallet::<hydradx_runtime::Runtime>::get_intent_id(deadline, increment_id);
		intent_ids.push(intent_id);
	}

	intent_ids
}
