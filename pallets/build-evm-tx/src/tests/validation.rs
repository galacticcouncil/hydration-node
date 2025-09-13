#[cfg(test)]
mod validation {
	use super::super::mock::*;

	#[test]
	fn validate_against_precomputed_alloy_rlp() {
		ExtBuilder::default().build().execute_with(|| {
			// Fixed transaction parameters
			let to_address = vec![
				0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
				0x11, 0x11, 0x11,
			];
			let value = 1000000000000000000u128; // 1 ETH
			let data = vec![0x12, 0x34, 0x56, 0x78];
			let nonce = 5u64;
			let gas_limit = 21000u64;
			let max_fee_per_gas = 20000000000u128; // 20 gwei
			let max_priority_fee_per_gas = 1000000000u128; // 1 gwei
			let chain_id = 1u64; // Ethereum mainnet

			// Pre-computed RLP using Alloy directly with the above parameters
			// This was generated using:
			// let tx = TxEip1559 {
			//     chain_id: 1,
			//     nonce: 5,
			//     gas_limit: 21000,
			//     max_fee_per_gas: 20000000000,
			//     max_priority_fee_per_gas: 1000000000,
			//     to: TxKind::Call(Address::from([0x11; 20])),
			//     value: U256::from(1000000000000000000u128),
			//     input: Bytes::from(vec![0x12, 0x34, 0x56, 0x78]),
			//     access_list: Default::default(),
			// };
			// let mut rlp = Vec::new();
			// tx.encode(&mut rlp);
			// prepend with 0x02 for EIP-1559
			let expected_rlp = vec![
				0x02, 0xf4, 0x01, 0x05, 0x84, 0x3b, 0x9a, 0xca, 0x00, 0x85, 0x04, 0xa8, 0x17, 0xc8, 0x00, 0x82, 0x52,
				0x08, 0x94, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
				0x11, 0x11, 0x11, 0x11, 0x11, 0x88, 0x0d, 0xe0, 0xb6, 0xb3, 0xa7, 0x64, 0x00, 0x00, 0x84, 0x12, 0x34,
				0x56, 0x78, 0xc0,
			];

			// Test the public function that other pallets call
			let returned_rlp = BuildEvmTx::build_evm_tx(
				Some(to_address),
				value,
				data,
				nonce,
				gas_limit,
				max_fee_per_gas,
				max_priority_fee_per_gas,
				chain_id,
			)
			.expect("Failed to build transaction");

			// Verify the returned RLP matches expected
			assert_eq!(
				returned_rlp,
				expected_rlp,
				"RLP mismatch!\nGot:      {:?}\nExpected: {:?}",
				alloy_primitives::hex::encode(&returned_rlp),
				alloy_primitives::hex::encode(&expected_rlp)
			);
		});
	}
}
