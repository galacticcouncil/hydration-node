#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_noop;
use hydradx_runtime::{Runtime, RuntimeCall, RuntimeOrigin};
use ismp::messaging::ConsensusMessage;
use sp_runtime::traits::Dispatchable;
use xcm_emulator::TestExt;

#[test]
fn ismp_update_parachain_consensus_does_not_panic_on_inherent_validation() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		// Invalid consensus message
		let consensus_msg = ConsensusMessage {
			consensus_proof: vec![1, 2, 3],
			consensus_state_id: [61, 92, 0, 0],
			signer: vec![1u8; 32],
		};

		let call = RuntimeCall::IsmpParachain(ismp_parachain::Call::update_parachain_consensus { data: consensus_msg });

		// Act
		let result = call.dispatch(RuntimeOrigin::none());

		// Assert
		assert!(result.is_err());
		assert_eq!(
			result.unwrap_err(),
			ismp_parachain::Error::<Runtime>::InvalidConsensusStateId.into()
		);
	});
}

#[test]
fn ismp_handle_unsigned_should_not_panic() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange

		let post_request = ismp::router::PostRequest {
			source: ismp::host::StateMachine::Evm(3892314112),
			dest: ismp::host::StateMachine::Substrate([0, 1, 128, 0]),
			nonce: 0,
			from: vec![],
			to: vec![],
			timeout_timestamp: 1,
			body: vec![],
		};

		let request = ismp::router::Request::Post(post_request);
		let msg = ismp::messaging::Message::Timeout(ismp::messaging::TimeoutMessage::Get {
			requests: vec![request],
		});

		assert_noop!(
			hydradx_runtime::Ismp::handle_unsigned(RuntimeOrigin::none(), vec![msg]),
			pallet_ismp::Error::<Runtime>::InvalidMessage
		);
	});
}
