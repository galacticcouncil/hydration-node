// Validation tests comparing with reference Ethereum implementation

#[cfg(test)]
mod validation {
	use super::super::mock::*;
	use crate::Event;
	use frame_support::assert_ok;
	
	// Alloy reference implementation
	use alloy_consensus::TxEip1559;
	use alloy_primitives::{Address, Bytes, TxKind, U256};
	use alloy_rlp::Encodable;
	use alloy_sol_types::{sol, SolCall};

	// Define ERC20 interface for encoding function calls
	sol! {
		interface IERC20 {
			function transfer(address to, uint256 amount) external returns (bool);
			function approve(address spender, uint256 amount) external returns (bool);
			function balanceOf(address account) external view returns (uint256);
		}
	}

	#[test]
	fn validate_erc20_transfer_transaction() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			
			// Transaction parameters
			let token_address = vec![0x42; 20]; // ERC20 token address
			let value = 0u128; // No ETH value for ERC20 transfer
			
			// Properly encode an ERC20 transfer call
			let recipient = Address::from([0x11; 20]);
			let amount = U256::from(1000000u64); // 1 million tokens
			let transfer_call = IERC20::transferCall {
				to: recipient,
				amount,
			};
			let encoded_data = transfer_call.abi_encode();
			
			let nonce = 5u64;
			let gas_limit = 65000u128;
			let max_fee_per_gas = 20000000000u128; // 20 gwei
			let max_priority_fee_per_gas = 1000000000u128; // 1 gwei
			let chain_id = 1u64; // Ethereum mainnet
			
			// Build transaction with our pallet
			assert_ok!(BuildEvmTx::build_evm_transaction(
				RuntimeOrigin::signed(1),
				Some(token_address.clone()),
				value,
				encoded_data.clone(),
				nonce,
				gas_limit,
				max_fee_per_gas,
				max_priority_fee_per_gas,
				chain_id,
			));
			
			// Get the RLP data from the emitted event
			let events = System::events();
			let our_rlp = events
				.iter()
				.find_map(|record| match &record.event {
					RuntimeEvent::BuildEvmTx(Event::TransactionBuilt { rlp_data, .. }) => {
						Some(rlp_data.clone())
					}
					_ => None,
				})
				.expect("TransactionBuilt event not found");
			
			// Build the same transaction with Alloy reference implementation
			let mut to_array = [0u8; 20];
			to_array.copy_from_slice(&token_address);
			
			let reference_tx = TxEip1559 {
				chain_id,
				nonce,
				gas_limit: gas_limit as u64,
				max_fee_per_gas: max_fee_per_gas as u128,
				max_priority_fee_per_gas: max_priority_fee_per_gas as u128,
				to: TxKind::Call(Address::from(to_array)),
				value: U256::from(value),
				input: Bytes::from(encoded_data),
				access_list: Default::default(),
			};
			
			// Encode the reference transaction
			let mut reference_rlp = Vec::new();
			reference_tx.encode(&mut reference_rlp);
			
			// Add EIP-1559 transaction type prefix (0x02)
			let mut prefixed_reference_rlp = vec![0x02];
			prefixed_reference_rlp.extend_from_slice(&reference_rlp);
			
			// Compare the RLP outputs
			assert_eq!(
				our_rlp, prefixed_reference_rlp,
				"ERC20 transfer RLP encoding mismatch!\nOur RLP: {:?}\nReference RLP: {:?}",
				hex::encode(&our_rlp),
				hex::encode(&prefixed_reference_rlp)
			);
		});
	}
}