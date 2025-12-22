#![cfg(test)]

use crate::polkadot_test_net::*;
use cumulus_pallet_parachain_system::Call;
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
