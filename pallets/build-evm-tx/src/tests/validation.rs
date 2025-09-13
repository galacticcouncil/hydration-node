// Validation test comparing pallet output with pre-computed Alloy RLP

#[cfg(test)]
mod validation {
	use super::super::mock::*;

	#[test]
	fn validate_against_precomputed_alloy_rlp() {
		new_test_ext().execute_with(|| {
			// Fixed transaction parameters
			let to_address = vec![
				0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
				0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
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
				0x02, // EIP-1559 transaction type
				0xf4, // RLP list header (correct length)
				0x01, // chain_id
				0x05, // nonce
				0x84, 0x3b, 0x9a, 0xca, 0x00, // max_priority_fee_per_gas (1 gwei)
				0x85, 0x04, 0xa8, 0x17, 0xc8, 0x00, // max_fee_per_gas (20 gwei)
				0x82, 0x52, 0x08, // gas_limit (21000)
				0x94, // to address prefix
				0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
				0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, // to address
				0x88, 0x0d, 0xe0, 0xb6, 0xb3, 0xa7, 0x64, 0x00, 0x00, // value (1 ETH)
				0x84, 0x12, 0x34, 0x56, 0x78, // data
				0xc0, // empty access_list
			];

			// Test the public function that other pallets call
			let returned_rlp = BuildEvmTx::build_evm_transaction(
				Some(to_address),
				value,
				data,
				nonce,
				gas_limit,
				max_fee_per_gas,
				max_priority_fee_per_gas,
				chain_id,
			).expect("Failed to build transaction");

			// Verify the returned RLP matches expected
			assert_eq!(
				returned_rlp, expected_rlp,
				"RLP mismatch!\nGot:      {:?}\nExpected: {:?}",
				alloy_primitives::hex::encode(&returned_rlp),
				alloy_primitives::hex::encode(&expected_rlp)
			);
		});
	}
}