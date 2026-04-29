#![cfg(test)]

use crate::polkadot_test_net::*;
use hydradx_runtime::CollatorRewards;
use pallet_session::SessionManager;
use pretty_assertions::assert_eq;
use xcm_emulator::TestExt;

#[test]
fn new_session_should_alternate_full_set_and_benched_set() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Invulnerables are stored sorted by `pallet_collator_selection`, so the
		// inner `CollatorSelection::new_session` returns them in sorted order.
		// `pallet_collator_rotation` then benches `(N / 2) % len` on odd sessions
		// only; even sessions pass the full set through unchanged.
		let collator1 = collators::invulnerables()[0].0.clone(); // d435...
		let collator2 = collators::invulnerables()[1].0.clone(); // 8eaf...
		let collator3 = collators::invulnerables()[2].0.clone(); // 90b5...
		let collator4 = collators::invulnerables()[3].0.clone(); // 6ebe...
		let collator5 = collators::invulnerables()[4].0.clone(); // ec5e...
		let collator6 = collators::invulnerables()[5].0.clone(); // 9c78...
		let collator7 = collators::invulnerables()[6].0.clone(); // a678...
		let collator8 = collators::invulnerables()[7].0.clone(); // 2433...
		let collator9 = collators::invulnerables()[8].0.clone(); // ee28...
		let collator10 = collators::invulnerables()[9].0.clone(); // da53...

		let full_sorted = vec![
			collator8.clone(),
			collator4.clone(),
			collator2.clone(),
			collator3.clone(),
			collator6.clone(),
			collator7.clone(),
			collator1.clone(),
			collator10.clone(),
			collator5.clone(),
			collator9.clone(),
		];

		// Session 0 (even): full set, no bench.
		assert_eq!(CollatorRewards::new_session(0).unwrap(), full_sorted);

		// Session 1 (odd, bench index 1/2 = 0): collator at sorted position 0.
		let mut session1 = full_sorted.clone();
		session1.remove(0);
		assert_eq!(CollatorRewards::new_session(1).unwrap(), session1);

		// Session 2 (even): full set returns.
		assert_eq!(CollatorRewards::new_session(2).unwrap(), full_sorted);

		// Session 3 (odd, bench index 3/2 = 1): collator at sorted position 1.
		let mut session3 = full_sorted.clone();
		session3.remove(1);
		assert_eq!(CollatorRewards::new_session(3).unwrap(), session3);
	});
}
