mod intents;
mod solution;
mod txorder;

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydradx_runtime::{Router, RuntimeOrigin, ICE};
use ice_solver::traits::ICESolver;
use pallet_ice::types::{BoundedInstructions, BoundedResolvedIntents, IntentId, ProposedSolution, Swap};
use primitives::{AccountId, AssetId, Moment};
use xcm_emulator::TestExt;

pub(crate) fn submit_intents(intents: Vec<(AccountId, Swap<AssetId>, Moment)>) -> Vec<IntentId> {
	let mut intent_ids = Vec::new();
	for (who, swap, deadline) in intents {
		let increment_id = pallet_ice::Pallet::<hydradx_runtime::Runtime>::next_incremental_id();
		assert_ok!(ICE::submit_intent(
			RuntimeOrigin::signed(who),
			swap,
			deadline,
			false,
			None,
			None,
		));
		let intent_id = pallet_ice::Pallet::<hydradx_runtime::Runtime>::get_intent_id(deadline, increment_id);
		intent_ids.push(intent_id);
	}

	intent_ids
}

pub(crate) fn solve_intents(
	intents: Vec<(IntentId, pallet_ice::types::Intent<AccountId, AssetId>)>,
) -> Result<ProposedSolution<AccountId, AssetId>, ()> {
	let solved = ice_solver::SimpleSolver::<hydradx_runtime::Runtime, Router, Router>::solve(intents)?;
	let resolved_intents = BoundedResolvedIntents::try_from(solved.intents).unwrap();
	let instructions = BoundedInstructions::try_from(solved.instructions).unwrap();
	Ok(ProposedSolution::<AccountId, AssetId> {
		intents: resolved_intents.clone(),
		instructions,
	})
}
