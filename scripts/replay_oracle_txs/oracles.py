#!/usr/bin/env python3

# tested with Python 3.12

import sys
import argparse
import time
from substrateinterface import SubstrateInterface
import rlp
from eth_utils import to_bytes

SOURCE_URL = "wss://rpc.hydradx.cloud"
TARGET_URL = "wss://1.lark.hydration.cloud"

IS_SUBMITTED = True

def integrity_check(source_substrate_interface, start_block, addresses, target_chain_nonces):
    """Verify existence of a block on the source chain with the same nonce as on the target chain"""
    source_chain_nonces = [get_nonce_at_block(source_substrate_interface, start_block, acc) for acc in addresses]
    print("---- source nonces: ", source_chain_nonces)
    print("---- target nonces: ", target_chain_nonces)
    return source_chain_nonces == target_chain_nonces

def get_nonce_at_block(substrate_interface, block_number, account_address):
    """Get account nonce at specific block number"""
    try:
        block_hash = substrate_interface.get_block_hash(block_number)
        result = substrate_interface.runtime_call(
            "EthereumRuntimeRPCApi",
            "account_basic",
            [account_address],
            block_hash=block_hash
        )
        # Note: API returns nonce under 'balance' field and balance under 'nonce' field
        nonce = result['balance']

        # Convert hex nonce to decimal if needed
        if isinstance(nonce, str) and nonce.startswith('0x'):
            nonce = int(nonce, 16)

        return nonce
    except Exception as e:
        print(f"Error getting nonce at block {block_number}: {e}")
        return None


def binary_search_nonce_block(substrate_interface, target_nonce, account_address):
    """Binary search to find block where nonce was target_nonce"""
    # Get current block as upper bound
    current_block = substrate_interface.get_block_number(substrate_interface.get_chain_head())

    # Set search bounds
    low = 1
    high = current_block

    print(f"Searching for block where nonce was {target_nonce}")

    while low <= high:
        mid = (low + high) // 2
        nonce = get_nonce_at_block(substrate_interface, mid, account_address)

        if nonce is None:
            print(f"Could not get nonce at block {mid}, trying next iteration")
            low = mid + 1
            continue

        if nonce == target_nonce:
            break
        elif nonce < target_nonce:
            low = mid + 1
        else:
            high = mid - 1

    result = mid

    print(f"Searching for latest block where nonce was {target_nonce}")
    while target_nonce == get_nonce_at_block(substrate_interface, result + 1, account_address):
        result += 1

    return result

def find_transactions_in_block(substrate_interface, block_number, account_addresses):
    """Find transaction in specific block"""
    try:
        result = substrate_interface.rpc_request(
            "eth_getBlockByNumber",
            [block_number, True]  # True to include full transaction details
        )

        if not result or 'result' not in result or not result['result'] or 'transactions' not in result['result']:
            return []

        # Check each transaction in the block
        transaction_list = []
        for tx in result['result']['transactions']:
            tx_from = tx.get('from', '').lower()

            # Check if the transaction is from our list of accounts
            if tx_from in account_addresses:
                transaction_list.append(tx)

        return transaction_list

    except Exception as e:
        print(f"Error processing block {block_number}: {e}")

def find_transactions_from_block_range(substrate_interface, target_interface, account_addresses, start_block, end_block=None):
    """Find all EVM transactions from account_addresses starting from start_block"""
    print("Running find_transactions_from_block_range()")
    if end_block is None:
        end_block = substrate_interface.get_block_number(substrate_interface.get_chain_head())

    if start_block > end_block:
        print("Start block is greater than end block, exiting...")
        return

    while True:
        print(f"Starting from block {start_block} to {end_block}")

        for block_num in range(start_block, end_block + 1):
            print(block_num, "/", end_block)
            transaction_list = find_transactions_in_block(substrate_interface, block_num, account_addresses)
            for tx in transaction_list:
                encoded_tx = reconstruct_raw_transaction(tx)
                tx_nonce = int(tx.get('nonce', ''), 16)
                print(tx_nonce, tx)
                if IS_SUBMITTED:
                    tx_hash = send_raw_transaction(target_interface, encoded_tx)
                    print(f"Transaction submitted with hash: {tx_hash}")
                    time.sleep(12)

        end_block_old = end_block
        end_block = substrate_interface.get_block_number(substrate_interface.get_chain_head())
        if end_block_old == end_block:
            break
        else:
            start_block = end_block_old + 1


def send_raw_transaction(substrate_interface, encoded_tx):
    """Submit a raw transaction to the network"""
    try:
        result = substrate_interface.rpc_request(
            "eth_sendRawTransaction",
            [encoded_tx]
        )
        return result.get('result')
    except Exception as e:
        print(f"Error sending raw transaction: {e}")
        print("Reconnecting...")
        substrate_interface.connect_websocket()
        try:
            result = substrate_interface.rpc_request(
                "eth_sendRawTransaction",
                [encoded_tx]
            )
            return result.get('result')
        except Exception as e:
            print(f"Error sending raw transaction: {e}")

def reconstruct_raw_transaction(tx_data):
    """
    Reconstruct a raw transaction from eth_getTransactionByHash response
    """

    # Extract transaction components
    nonce = tx_data['nonce']
    gas_price = tx_data['gasPrice']
    gas = tx_data['gas']
    to_address = tx_data['to']
    value = tx_data['value']
    data = tx_data['input']
    v = tx_data['v']
    r = tx_data['r']
    s = tx_data['s']

    # Convert to proper format for RLP encoding
    # For legacy transactions, the structure is:
    # [nonce, gasPrice, gasLimit, to, value, data, v, r, s]

    transaction_fields = [
        int(nonce, 16),
        int(gas_price, 16),
        int(gas, 16),
        to_bytes(hexstr=to_address) if to_address else b'',
        int(value, 16),
        to_bytes(hexstr=data),
        int(v, 16),
        int(r, 16),
        int(s, 16),
    ]

    # RLP encode the transaction
    raw_transaction = rlp.encode(transaction_fields)

    return '0x' + raw_transaction.hex()

def find_latest_block_with_nonce(interface, nonce, address):
    block_number = binary_search_nonce_block(interface, nonce, address)

    print(f"Latest block for account {address} with nonce {nonce} is {block_number}")
    return block_number

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('addresses', metavar='N', type=str, nargs='*',
                        help='a list of addresses')
    parser.add_argument("--no-integrity-check", help="disable integrity check",
                        action="store_true")
    args = parser.parse_args()

    addresses = []
    if not args.addresses:
        addresses = ["0x33a5e905fB83FcFB62B0Dd1595DfBc06792E054e", "0xFf0c624016c873d359DdE711B42A2F475a5a07d3"]
        # 12ZuLmURGvTLdUHiyXBPQpnAignzipZkWQhewbPhunKMQX6Q, 12ZuLmV82mQTfL2E2CgWb8TvXQhBJ7Y7yqidrL1QxWQFQAVP
    else:
        addresses = args.addresses

    addresses = list(map(str.lower, addresses))

    # Connect to Hydration RPC endpoint
    source_substrate_interface = SubstrateInterface(url=SOURCE_URL)
    # Connect to the endpoint where we want to resubmit the transactions
    target_substrate_interface = SubstrateInterface(url=TARGET_URL)

    current_block = target_substrate_interface.get_block_number(target_substrate_interface.get_chain_head())
    nonces = [get_nonce_at_block(target_substrate_interface, current_block, acc) for acc in addresses]
    print("---- result nonces: ", nonces)

    blocks = [find_latest_block_with_nonce(source_substrate_interface, nonce, acc) for acc, nonce in zip(addresses, nonces)]
    print("---- result blocks: ", blocks)

    # The block number from which we start to search for new transactions
    start_block = min(blocks) + 1
    print("The block number from which we start to search for new transactions: ", start_block)

    if not args.no_integrity_check:
        if not integrity_check(source_substrate_interface, start_block - 1, addresses, nonces):
            print("Integrity check failed. Exiting.")
            sys.exit()

    # resubmit all past transactions from the source node
    find_transactions_from_block_range(source_substrate_interface, target_substrate_interface, addresses, start_block)

    def subscription_handler(obj, update_nr, subscription_id):
        print("new block: ", obj['header']['number'])
        tx_list = find_transactions_in_block(source_substrate_interface, obj['header']['number'], addresses)
        for tx in tx_list:
            tx_nonce = int(tx.get('nonce', ''), 16)
            encoded_tx = reconstruct_raw_transaction(tx)
            print(tx_nonce, encoded_tx)
            if IS_SUBMITTED:
                tx_hash = send_raw_transaction(target_substrate_interface, encoded_tx)
                print(f"Transaction submitted with nonce {tx_nonce} and hash: {tx_hash}")


    # wait for new transactions and resubmit them
    while True:
        print("Waiting for new transactions...")
        try:
            source_substrate_interface.subscribe_block_headers(subscription_handler)
        except Exception as e:
            print("Reconnecting...")
            source_substrate_interface.connect_websocket()

if __name__ == "__main__":
    main()

