# Integration Tests

This directory contains integration tests designed to validate operational extrinsics and various node behaviors within a local Polkadot/Substrate-based network. The tests leverage [Zombienet](https://github.com/paritytech/zombienet) to orchestrate nodes and perform automated checks.

## Overview

The tests are defined in `.zndsl` files and corresponding scripts in the `scripts` directory. They perform actions such as:

- Sending normal and operational (high-priority) extrinsics.
- Verifying node metrics and logs (e.g., ensuring the node is not major-syncing, checking that certain log messages appear).
- Running custom JavaScript scripts that interact with the node's RPC (using `@polkadot/api`) to perform extrinsics and queries.

The tests are designed to ensure:

- The network launches successfully with the expected node roles.
- Operational class extrinsics (like pausing a transaction or toggling asset tradability) execute successfully.
- Normal and operational extrinsics both get included in blocks.
- Queries return the expected results (e.g., checking asset tradable state).

## File Structure

- **`operational_class_test.zndsl`**: A Zombienet DSL file that defines the test scenario including:
  - The network specification (which nodes, what chain spec).
  - The metrics and log checks to perform.
  - The scripts to run as test steps.

- **`scripts/`**: Contains the custom JavaScript and shell scripts that the `.zndsl` file invokes.
  - `send_normal_tx.js` & `send_normal_tx.sh`: Send a series of normal (non-operational) transactions.
  - `send_operational_extrinsic.js` & `send_operational_extrinsic.sh`: Send an operational extrinsic, such as pausing a transaction.
  - `send_asset_tradable_operational.js` & `send_asset_tradable_operational.sh`: Adjust asset tradability via an operational extrinsic.
  - `query_asset_state.js` & `query_asset_state.sh`: Query on-chain state (e.g., whether an asset is tradable).
  - `send_many_normal_txs.js` & `send_many_normal_txs.sh`: Stress test by sending many normal transactions.
  
  The `.sh` scripts typically create and run the `.js` files inside the ephemeral environment spawned by Zombienet, ensuring that the code is executed against the correct node RPC endpoint.  
  The `.js` scripts contain logic using `@polkadot/api` for extrinsic submission and state queries, including assertions for success or failure.

## Running the Tests

1. **Prerequisites**:
   - [Node.js](https://nodejs.org/en/) and `npm` installed.
   - `@polkadot/api` and other required packages installed (usually `npm install` in the `tests/scripts` directory).
   - [Zombienet](https://github.com/paritytech/zombienet) installed. Ensure you have the correct binary (`zombienet`) available in your PATH.

2. **Starting the Test**:
   From the `integration-tests` or the parent directory:
   ```bash
   ./zombienet test tests/operational_class_test.zndsl --provider native

