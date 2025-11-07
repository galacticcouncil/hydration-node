// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";

contract GasVoucher is ERC20, AccessControl {
    bytes32 public constant FAUCET_ROLE = keccak256("FAUCET_ROLE");

    constructor() ERC20("Gas Voucher", "GVCH") {
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
    }

    function setFaucet(address _faucet) external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(_faucet != address(0), "zero faucet");
        _grantRole(FAUCET_ROLE, _faucet);
    }

    function faucetMint(
        address to,
        uint256 amount
    ) external onlyRole(FAUCET_ROLE) {
        require(to != address(0), "zero to");
        _mint(to, amount);
    }

    function faucetBurnFrom(
        address from,
        uint256 amount
    ) external onlyRole(FAUCET_ROLE) {
        _burn(from, amount);
    }
}
