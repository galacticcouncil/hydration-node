// Validation tests comparing with reference Ethereum implementation

#[cfg(test)]
mod validation {
	use super::super::mock::*;
	use crate::Event;
	use frame_support::assert_ok;
	
	// Alloy reference implementation
	use alloy_consensus::TxEip1559;
	use alloy_primitives::{Address, Bytes, TxKind, U256, FixedBytes};
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

	// Define a simple Uniswap V2 Router interface for complex calls
	sol! {
		interface IUniswapV2Router {
			function swapExactETHForTokens(
				uint256 amountOutMin,
				address[] calldata path,
				address to,
				uint256 deadline
			) external payable returns (uint256[] memory amounts);
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

	#[test]
	fn validate_contract_creation_with_real_bytecode() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			
			// Contract creation parameters
			let value = 0u128;
			
			// Real minimal contract bytecode (stores a value and allows retrieval)
			// This is the bytecode for a simple storage contract:
			// contract SimpleStorage {
			//     uint256 public storedData;
			//     constructor(uint256 initialValue) {
			//         storedData = initialValue;
			//     }
			// }
			// Bytecode includes constructor logic and runtime code
			let init_code = hex::decode(
				"608060405234801561001057600080fd5b506040516101583803806101588339818101604052810190610032919061007a565b80600081905550506100a7565b600080fd5b6000819050919050565b61005781610044565b811461006257600080fd5b50565b6000815190506100748161004e565b92915050565b6000602082840312156100905761008f61003f565b5b600061009e84828501610065565b91505092915050565b60a2806100b66000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c80632a1afcd914602d575b600080fd5b60336047565b604051603e9190605d565b60405180910390f35b60005481565b6000819050919050565b6057816046565b82525050565b6000602082019050607060008301846050565b9291505056fea2646970667358221220"
			).expect("Valid hex");
			
			let nonce = 0u64;
			let gas_limit = 200000u128;
			let max_fee_per_gas = 30000000000u128; // 30 gwei
			let max_priority_fee_per_gas = 2000000000u128; // 2 gwei
			let chain_id = 137u64; // Polygon
			
			// Build contract creation with our pallet
			assert_ok!(BuildEvmTx::build_evm_transaction(
				RuntimeOrigin::signed(2),
				None, // No to_address for contract creation
				value,
				init_code.clone(),
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
			let reference_tx = TxEip1559 {
				chain_id,
				nonce,
				gas_limit: gas_limit as u64,
				max_fee_per_gas: max_fee_per_gas as u128,
				max_priority_fee_per_gas: max_priority_fee_per_gas as u128,
				to: TxKind::Create, // Contract creation
				value: U256::from(value),
				input: Bytes::from(init_code),
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
				"Contract creation RLP encoding mismatch!\nOur RLP: {:?}\nReference RLP: {:?}",
				hex::encode(&our_rlp),
				hex::encode(&prefixed_reference_rlp)
			);
		});
	}

	#[test]
	fn validate_uniswap_swap_transaction() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			
			// Uniswap V2 Router address (mainnet)
			let router_address = hex::decode("7a250d5630B4cF539739dF2C5dAcb4c659F2488D")
				.expect("Valid hex");
			
			// ETH to token swap parameters
			let value = 1000000000000000000u128; // 1 ETH
			
			// Encode swapExactETHForTokens call
			let amount_out_min = U256::from(1000000000u64); // Minimum tokens to receive
			let weth_address = Address::from(FixedBytes::from_slice(&hex::decode("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").expect("Valid hex")));
			let token_address = Address::from(FixedBytes::from_slice(&hex::decode("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").expect("Valid hex"))); // USDC
			let path = vec![weth_address, token_address];
			let recipient = Address::from([0x33; 20]);
			let deadline = U256::from(1800000000u64); // Unix timestamp
			
			let swap_call = IUniswapV2Router::swapExactETHForTokensCall {
				amountOutMin: amount_out_min,
				path,
				to: recipient,
				deadline,
			};
			let encoded_data = swap_call.abi_encode();
			
			let nonce = 42u64;
			let gas_limit = 200000u128;
			let max_fee_per_gas = 50000000000u128; // 50 gwei
			let max_priority_fee_per_gas = 3000000000u128; // 3 gwei
			let chain_id = 1u64; // Ethereum mainnet
			
			// Build transaction with our pallet
			assert_ok!(BuildEvmTx::build_evm_transaction(
				RuntimeOrigin::signed(3),
				Some(router_address.clone()),
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
			to_array.copy_from_slice(&router_address);
			
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
				"Uniswap swap RLP encoding mismatch!\nOur RLP: {:?}\nReference RLP: {:?}",
				hex::encode(&our_rlp),
				hex::encode(&prefixed_reference_rlp)
			);
		});
	}
}