// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

/// Test receiver that always reverts, used to assert the forward rolls back and the owner is left whole.
contract RevertingReceiver {
    function execute(
        address user,
        uint256 intentId,
        address assetIn,
        uint256 amountIn,
        address assetOut,
        uint256 amountOut,
        bytes calldata data
    ) external returns (bytes4) {
        revert("receiver rejected");
    }
}
