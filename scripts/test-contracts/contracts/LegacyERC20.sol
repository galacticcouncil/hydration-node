// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract LegacyERC20 is ERC20 {
    constructor() ERC20("Legacy", "LEGACY") {
        _mint(msg.sender, 1_000_000_000 * 10 ** decimals());
    }

    // Old-style error handling: require(..., "message")
    function transfer(address to, uint256 value) public override returns (bool) {
        address owner = _msgSender();

        // mimic classic OZ messages for backward-compat tests
        require(to != address(0), "ERC20: transfer to the zero address");

        uint256 fromBalance = balanceOf(owner);
        require(fromBalance >= value, "ERC20: transfer amount exceeds balance");

        // Use the modern internal hook to move balances + emit Transfer
        _update(owner, to, value);
        return true;
    }
}
