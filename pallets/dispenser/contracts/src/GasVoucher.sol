// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";
import {IGasVoucher} from "./interfaces/IGasVoucher.sol";
import "./utils/Errors.sol";

/// @title GasVoucher
/// @notice ERC20 IOU token representing claims on future gas from a faucet.
/// @dev
/// - Uses AccessControl for admin + faucet permissions.
/// - Faucet contracts with `FAUCET_ROLE` can mint/burn vouchers on behalf of users.
/// - Admin (DEFAULT_ADMIN_ROLE) can assign faucet contracts via `setFaucet`.
contract GasVoucher is ERC20, AccessControl, IGasVoucher {
    /// @inheritdoc IGasVoucher
    bytes32 public constant FAUCET_ROLE = keccak256("FAUCET_ROLE");

    /// @notice Deploys the GasVoucher token.
    /// @dev Grants DEFAULT_ADMIN_ROLE to the deployer.
    constructor(address _admin) ERC20("Gas Voucher", "GVCH") {
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
    function faucetMint(
        address to,
        uint256 amount
    ) external override onlyRole(FAUCET_ROLE) {
        if (to == address(0)) {
            revert ZeroAddress();
        }

        _mint(to, amount);
    }

    /// @inheritdoc IGasVoucher
    function faucetBurnFrom(
        address from,
        uint256 amount
    ) external override onlyRole(FAUCET_ROLE) {
        // No zero-address check here: burning from zero would already be
        // invalid in ERC20 semantics (it will underflow / revert internally).
        _burn(from, amount);
    }
}
