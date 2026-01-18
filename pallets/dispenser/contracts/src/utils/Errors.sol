// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

/// @notice Thrown when a non-MPC caller invokes an MPC-only function.
error NotMPC();

/// @notice Thrown when a zero address is used where it is not allowed.
error ZeroAddress();

/// @notice Thrown when a zero amount is used but a positive value is required.
error ZeroAmount();

/// @notice Thrown when the faucet does not have enough ETH for a redeem/withdraw.
error FaucetLowBalance();

/// @notice Thrown when a direct `fund` call would push the faucet below the threshold.
error FaucetBelowThreshold();

/// @notice Thrown when an ETH transfer fails.
error EthTransferFailed();
