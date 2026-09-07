use crate::*;
use codec::Decode;
use frame_support::assert_ok;
use frame_support::traits::Hooks;
use hydradx_traits::lazy_executor::{ForwardAction, Source};
use pretty_assertions::assert_eq;
use sp_core::offchain::testing::PoolState;
use sp_std::sync::Arc;
use tests::mock::*;

fn forward() -> ForwardAction {
	ForwardAction {
		contract: contract_address(),
		intent_id: 1,
		asset_in: HDX,
		amount_in: 10 * UNIT,
		asset_out: DOT,
		amount_out: 100 * UNIT,
		data: Default::default(),
	}
}

fn queue_forwards(n: u128) {
	for i in 0..n {
		assert_ok!(LazyExecutor::add_to_queue(Source::ICE(i), ALICE, forward()));
	}
}

/// The queue ids the worker asked the runtime to dispatch, in submission order.
fn submitted_ids(pool_state: &Arc<parking_lot::RwLock<PoolState>>) -> Vec<u128> {
	pool_state
		.read()
		.transactions
		.iter()
		.map(|tx| {
			let extrinsic = Extrinsic::decode(&mut &tx[..]).expect("offchain worker submitted a decodable extrinsic");
			match extrinsic.function {
				RuntimeCall::LazyExecutor(LazyExecutorCall::dispatch_top { id }) => id,
				other => panic!("expected dispatch_top, got {other:?}"),
			}
		})
		.collect()
}

/// A consumer that submits one intent and waits for its callback has a queue depth of exactly
/// one, which the worker used to skip forever: it probed `cursor + 1` while `dispatch_top`
/// consumes the job at `cursor`.
#[test]
fn offchain_worker_should_submit_dispatch_top_when_a_single_forward_is_queued() {
	let (mut ext, pool_state) = ExtBuilder::new().build_with_pool();

	ext.execute_with(|| {
		//Arrange
		queue_forwards(1);
		assert_eq!(LazyExecutor::dispatch_next_id(), 0);

		//Act
		LazyExecutor::offchain_worker(1);

		//Assert
		assert_eq!(submitted_ids(&pool_state), vec![0]);
	});
}

#[test]
fn offchain_worker_should_submit_nothing_when_queue_is_empty() {
	let (mut ext, pool_state) = ExtBuilder::new().build_with_pool();

	ext.execute_with(|| {
		//Act
		LazyExecutor::offchain_worker(1);

		//Assert
		assert_eq!(submitted_ids(&pool_state), Vec::<u128>::new());
	});
}

/// The newest entry must be dispatched too — the old loop always left it behind, so the queue
/// permanently lagged one job even when it was never empty.
#[test]
fn offchain_worker_should_submit_for_every_forward_when_multiple_are_queued() {
	let (mut ext, pool_state) = ExtBuilder::new().build_with_pool();

	ext.execute_with(|| {
		//Arrange
		queue_forwards(3);

		//Act
		LazyExecutor::offchain_worker(1);

		//Assert
		assert_eq!(submitted_ids(&pool_state), vec![0, 1, 2]);
	});
}

#[test]
fn offchain_worker_should_resume_from_cursor_when_earlier_forwards_were_dispatched() {
	let (mut ext, pool_state) = ExtBuilder::new()
		.with_tokens(vec![(ALICE, DOT, 1_000 * UNIT)])
		.build_with_pool();

	ext.execute_with(|| {
		//Arrange
		queue_forwards(3);
		set_evm_outcome(EvmOutcome::SucceedCorrectAck);
		assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none(), 0));
		assert_eq!(LazyExecutor::dispatch_next_id(), 1);

		//Act
		LazyExecutor::offchain_worker(2);

		//Assert
		assert_eq!(submitted_ids(&pool_state), vec![1, 2]);
	});
}

#[test]
fn offchain_worker_should_stop_at_max_txs_per_block_when_queue_is_longer() {
	let (mut ext, pool_state) = ExtBuilder::new().build_with_pool();

	ext.execute_with(|| {
		//Arrange
		assert_eq!(LazyExecutor::max_txs_per_block(), 10);
		queue_forwards(12);

		//Act
		LazyExecutor::offchain_worker(1);

		//Assert
		assert_eq!(submitted_ids(&pool_state), (0..10).collect::<Vec<u128>>());
	});
}
