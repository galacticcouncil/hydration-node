// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {IGasVoucher} from "./IGasVoucher.sol";

/// @title IGasFaucet
/// @notice Interface for GasFaucet implementations.
/// @dev Contains events and external function signatures.
interface IGasFaucet {
    // ========= Events =========

    /// @notice Emitted when ETH is sent directly from the faucet.
    /// @param to Recipient of the ETH.
    /// @param amountWei Amount of ETH sent (in wei).
    event Funded(address indexed to, uint256 amountWei);

    /// @notice Emitted when vouchers are issued instead of sending ETH.
    /// @param to Recipient of the vouchers.
    /// @param amountWei Amount of ETH-equivalent represented by the vouchers (in wei).
    event VoucherIssued(address indexed to, uint256 amountWei);

    /// @notice Emitted when vouchers are redeemed for ETH.
    /// @param redeemer Address redeeming the vouchers.
    /// @param amountWei Amount of ETH redeemed (in wei).
    event Redeemed(address indexed redeemer, uint256 amountWei);

    /// @notice Emitted when the owner withdraws ETH from the faucet.
    /// @param to Recipient of the withdrawn ETH.
    /// @param amountWei Amount of ETH withdrawn (in wei).
    event Withdrawn(address indexed to, uint256 amountWei);

    /// @notice Emitted when the MPC address is updated.
    /// @param newMpc New MPC address.
    event MPCUpdated(address indexed newMpc);

    /// @notice Emitted when the minimum ETH threshold is updated.
    /// @param newThreshold New threshold (in wei).
    event ThresholdUpdated(uint256 newThreshold);

    /// @notice Emitted when the voucher contract address is updated.
    /// @param newVoucher New `GasVoucher` contract address.
    event VoucherUpdated(address indexed newVoucher);

    // ========= View functions (optional in interface, but nice to have) =========

    /// @notice Returns the MPC address allowed to call `fund`.
    function mpc() external view returns (address);

    /// @notice Returns the minimum ETH threshold (in wei).
    function minEthThreshold() external view returns (uint256);

    /// @notice Returns the current voucher contract.
    function voucher() external view returns (IGasVoucher);

    // ========= Admin Functions =========

    /// @notice Updates the MPC address.
    /// @param _mpc New MPC address.
    function setMPC(address _mpc) external;

    /// @notice Updates the minimum ETH threshold.
    /// @param _thresholdWei New minimum ETH threshold (in wei).
    function setMinEthThreshold(uint256 _thresholdWei) external;

    /// @notice Updates the voucher contract used for IOUs.
    /// @param _voucher Address of the new `GasVoucher` contract.
    function setVoucher(address _voucher) external;

    // ========= Core Logic =========

    /// @notice Funds a recipient with ETH or vouchers, depending on faucet balance.
    /// @param to Recipient address to receive ETH or vouchers.
    /// @param amountWei Requested amount (in wei).
    function fund(address to, uint256 amountWei) external;

    /// @notice Redeems vouchers for ETH from the faucet.
    /// @param amountWei Amount to redeem (in wei).
    function redeem(uint256 amountWei) external;

    /// @notice Withdraws ETH from the faucet to a specified address.
    /// @param to Recipient of the withdrawn ETH.
    /// @param amountWei Amount of ETH to withdraw (in wei).
    function withdraw(address payable to, uint256 amountWei) external;
}
