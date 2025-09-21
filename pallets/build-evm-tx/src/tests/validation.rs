#[cfg(test)]
mod validation {
	use super::super::mock::*;
	use ethereum::AccessListItem;
	use sp_core::{H160, H256};

	#[test]
	fn eip1559_call_basic_no_access_list_matches_reference_rlp() {
		ExtBuilder::default().build().execute_with(|| {
			let to_address = H160::from([0x11u8; 20]);
			let value = 1000000000000000000u128; // 1 ETH
			let data = vec![0x12, 0x34, 0x56, 0x78];
			let nonce = 5u64;
			let gas_limit = 21000u64;
			let max_fee_per_gas = 20000000000u128; // 20 gwei
			let max_priority_fee_per_gas = 1000000000u128; // 1 gwei
			let chain_id = 1u64; // Ethereum mainnet

			// Pre-computed RLP for EIP-1559 transaction with the above parameters
			// This was generated using:
			// let tx = EIP1559TransactionMessage {
			//     chain_id: 1,
			//     nonce: U256::from(5),
			//     gas_limit: U256::from(21000),
			//     max_fee_per_gas: U256::from(20000000000),
			//     max_priority_fee_per_gas: U256::from(1000000000),
			//     action: TransactionAction::Call(H160::from([0x11; 20])),
			//     value: U256::from(1000000000000000000u128),
			//     input: vec![0x12, 0x34, 0x56, 0x78],
			//     access_list: vec![],
			// };
			// let encoded = rlp::encode(&tx);
			// prepend with 0x02 for EIP-1559
			let expected_rlp = vec![
				0x02, 0xf4, 0x01, 0x05, 0x84, 0x3b, 0x9a, 0xca, 0x00, 0x85, 0x04, 0xa8, 0x17, 0xc8, 0x00, 0x82, 0x52,
				0x08, 0x94, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
				0x11, 0x11, 0x11, 0x11, 0x11, 0x88, 0x0d, 0xe0, 0xb6, 0xb3, 0xa7, 0x64, 0x00, 0x00, 0x84, 0x12, 0x34,
				0x56, 0x78, 0xc0,
			];

			let returned_rlp = BuildEvmTx::build_evm_tx(
				RuntimeOrigin::signed(1u64),
				Some(to_address),
				value,
				data,
				nonce,
				gas_limit,
				max_fee_per_gas,
				max_priority_fee_per_gas,
				Vec::new(),
				chain_id,
			)
			.expect("Failed to build transaction");

			assert_eq!(
				returned_rlp,
				expected_rlp,
				"RLP mismatch!\nGot:      {:?}\nExpected: {:?}",
				hex::encode(&returned_rlp),
				hex::encode(&expected_rlp)
			);
		});
	}

	#[test]
	fn eip1559_contract_creation_with_data_matches_reference_rlp() {
		ExtBuilder::default().build().execute_with(|| {
			let value = 0u128;
			let data = vec![0xab, 0xcd];
			let nonce = 10u64;
			let gas_limit = 53000u64;
			let max_fee_per_gas = 30_000_000_000u128; // 30 gwei
			let max_priority_fee_per_gas = 2_000_000_000u128; // 2 gwei
			let chain_id = 1u64;

			// Reference generation (using `ethereum` crate):
			// let tx = EIP1559TransactionMessage {
			//     chain_id: 1,
			//     nonce: U256::from(10),
			//     gas_limit: U256::from(53000),
			//     max_fee_per_gas: U256::from(30_000_000_000u128),
			//     max_priority_fee_per_gas: U256::from(2_000_000_000u128),
			//     action: TransactionAction::Create,
			//     value: U256::from(0u128),
			//     input: vec![0xab, 0xcd],
			//     access_list: vec![],
			// };
			// let mut expected_rlp = vec![0x02];
			// expected_rlp.extend_from_slice(&rlp::encode(&tx));

			let expected_rlp = vec![
				0x02, 0xd6, 0x01, 0x0a, 0x84, 0x77, 0x35, 0x94, 0x00, 0x85, 0x06, 0xfc, 0x23, 0xac, 0x00, 0x82, 0xcf,
				0x08, 0x80, 0x80, 0x82, 0xab, 0xcd, 0xc0,
			];

			let returned_rlp = BuildEvmTx::build_evm_tx(
				RuntimeOrigin::signed(1u64),
				None,
				value,
				data,
				nonce,
				gas_limit,
				max_fee_per_gas,
				max_priority_fee_per_gas,
				Vec::new(),
				chain_id,
			)
			.expect("Failed to build transaction");

			assert_eq!(
				returned_rlp,
				expected_rlp,
				"RLP mismatch!\nGot:      {:?}\nExpected: {:?}",
				hex::encode(&returned_rlp),
				hex::encode(&expected_rlp)
			);
		});
	}

	#[test]
	fn eip1559_call_with_non_empty_access_list_matches_reference_rlp() {
		ExtBuilder::default().build().execute_with(|| {
			let to_address = H160::from([0x22u8; 20]);
			let value = 1u128;
			let data = Vec::new();
			let nonce = 3u64;
			let gas_limit = 500_000u64;
			let max_fee_per_gas = 40_000_000_000u128; // 40 gwei
			let max_priority_fee_per_gas = 1_500_000_000u128; // 1.5 gwei
			let chain_id = 1u64;

			let access_list = vec![AccessListItem {
				address: H160::from([0x33u8; 20]),
				storage_keys: vec![H256::from([0x44u8; 32]), H256::from([0x55u8; 32])],
			}];

			// Reference generation (using `ethereum` crate):
			// let tx = EIP1559TransactionMessage {
			//     chain_id: 1,
			//     nonce: U256::from(3),
			//     gas_limit: U256::from(500_000),
			//     max_fee_per_gas: U256::from(40_000_000_000u128),
			//     max_priority_fee_per_gas: U256::from(1_500_000_000u128),
			//     action: TransactionAction::Call(H160::from([0x22; 20])),
			//     value: U256::from(1u128),
			//     input: vec![],
			//     access_list: vec![AccessListItem {
			//         address: H160::from([0x33; 20]),
			//         storage_keys: vec![
			//             H256::from([0x44; 32]),
			//             H256::from([0x55; 32]),
			//         ],
			//     }],
			// };
			// let mut expected_rlp = vec![0x02];
			// expected_rlp.extend_from_slice(&rlp::encode(&tx));

			let expected_rlp = vec![
				0x02, 0xF8, 0x85, 0x01, 0x03, 0x84, 0x59, 0x68, 0x2F, 0x00, 0x85, 0x09, 0x50, 0x2F, 0x90, 0x00, 0x83,
				0x07, 0xA1, 0x20, 0x94, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22,
				0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x01, 0x80, 0xF8, 0x5B, 0xF8, 0x59, 0x94, 0x33, 0x33, 0x33,
				0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33,
				0xF8, 0x42, 0xA0, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44,
				0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44,
				0x44, 0xA0, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
				0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
			];

			let returned_rlp = BuildEvmTx::build_evm_tx(
				RuntimeOrigin::signed(1u64),
				Some(to_address),
				value,
				data,
				nonce,
				gas_limit,
				max_fee_per_gas,
				max_priority_fee_per_gas,
				access_list,
				chain_id,
			)
			.expect("Failed to build transaction");

			assert_eq!(
				returned_rlp,
				expected_rlp,
				"RLP mismatch!\nGot:      {:?}\nExpected: {:?}",
				hex::encode(&returned_rlp),
				hex::encode(&expected_rlp)
			);
		});
	}
}
