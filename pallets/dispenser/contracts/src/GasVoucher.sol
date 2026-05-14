// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";
import {Pausable} from "@openzeppelin/contracts/utils/Pausable.sol";
import {IGasVoucher} from "./interfaces/IGasVoucher.sol";
import "./utils/Errors.sol";

/// @title GasVoucher
/// @notice ERC20 IOU token representing claims on future gas from a faucet.
/// @dev
/// - Uses AccessControl for admin + faucet permissions.
/// - Faucet contracts with `FAUCET_ROLE` can mint/burn vouchers on behalf of users.
/// - Admin (DEFAULT_ADMIN_ROLE) can assign faucet contracts via `setFaucet`.
/// - Pausable for emergency situations.
contract GasVoucher is ERC20, AccessControl, Pausable, IGasVoucher {
    /// @inheritdoc IGasVoucher
    bytes32 public constant FAUCET_ROLE = keccak256("FAUCET_ROLE");

    /// @notice Deploys the GasVoucher token.
    /// @dev Grants DEFAULT_ADMIN_ROLE to the deployer.
    /// @param _admin Address to receive admin role. Must not be zero address.
    constructor(address _admin) ERC20("Gas Voucher", "GVCH") {
        if (_admin == address(0)) {
            revert ZeroAddress();
        }
        _grantRole(DEFAULT_ADMIN_ROLE, _admin);
    }

    /// @inheritdoc IGasVoucher
    function setFaucet(
        address _faucet
    ) external override onlyRole(DEFAULT_ADMIN_ROLE) {
        if (_faucet == address(0)) {
            revert ZeroAddress();
        }

        _grantRole(FAUCET_ROLE, _faucet);
        emit FaucetUpdated(_faucet);
    }

    /// @inheritdoc IGasVoucher
    function revokeFaucet(
        address _faucet
    ) external override onlyRole(DEFAULT_ADMIN_ROLE) {
        _revokeRole(FAUCET_ROLE, _faucet);
        emit FaucetRevoked(_faucet);
    }

    /// @notice Pauses all token transfers and minting/burning.
    /// @dev Only callable by admin.
    function pause() external onlyRole(DEFAULT_ADMIN_ROLE) {
        _pause();
    }

    /// @notice Unpauses all token transfers and minting/burning.
    /// @dev Only callable by admin.
    function unpause() external onlyRole(DEFAULT_ADMIN_ROLE) {
        _unpause();
    }

    /// @inheritdoc IGasVoucher
    function faucetMint(
        address to,
        uint256 amount
    ) external override onlyRole(FAUCET_ROLE) whenNotPaused {
        if (to == address(0)) {
            revert ZeroAddress();
        }

        _mint(to, amount);
    }

    /// @inheritdoc IGasVoucher
    function faucetBurnFrom(
        address from,
        uint256 amount
    ) external override onlyRole(FAUCET_ROLE) whenNotPaused {
        // No zero-address check here: burning from zero would already be
        // invalid in ERC20 semantics (it will underflow / revert internally).
        _burn(from, amount);
    }

    /// @dev Hook that is called before any token transfer. Respects pause state.
    function _update(
        address from,
        address to,
        uint256 value
    ) internal virtual override whenNotPaused {
        super._update(from, to, value);
    }
}
