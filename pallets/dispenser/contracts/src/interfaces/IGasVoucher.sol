// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

/// @title IGasVoucher
/// @notice Interface for GasVoucher implementations used as IOU tokens for gas.
/// @dev Extends IERC20 with faucet-specific hooks.
interface IGasVoucher is IERC20 {
    /// @notice Emitted when a new faucet address is granted the faucet role.
    /// @param faucet Address that is now authorized as a faucet.
    event FaucetUpdated(address indexed faucet);

    /// @notice Emitted when a faucet address has its role revoked.
    /// @param faucet Address that no longer has faucet permissions.
    event FaucetRevoked(address indexed faucet);

    /// @notice Role identifier used to mark authorized faucet contracts.
    function FAUCET_ROLE() external view returns (bytes32);

    /// @notice Grants faucet permissions to `_faucet`.
    /// @dev Expected to be restricted to an admin (e.g. DEFAULT_ADMIN_ROLE).
    /// @param _faucet Address of the faucet contract.
    function setFaucet(address _faucet) external;

    /// @notice Revokes faucet permissions from `_faucet`.
    /// @dev Expected to be restricted to an admin (e.g. DEFAULT_ADMIN_ROLE).
    /// @param _faucet Address of the faucet contract to revoke.
    function revokeFaucet(address _faucet) external;

    /// @notice Mints voucher tokens to `to`.
    /// @dev Expected to be restricted to a faucet contract.
    /// @param to Recipient of the minted vouchers.
    /// @param amount Amount of vouchers to mint.
    function faucetMint(address to, uint256 amount) external;

    /// @notice Burns voucher tokens from `from`.
    /// @dev Expected to be restricted to a faucet contract.
    /// @param from Address whose vouchers are burned.
    /// @param amount Amount of vouchers to burn.
    function faucetBurnFrom(address from, uint256 amount) external;
}
