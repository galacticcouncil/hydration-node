// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract WeirdToken is ERC20 {
    constructor() ERC20("Weird", "WEIRD") {
        _mint(msg.sender, 1_000_000_000 * 10 ** decimals());
    }

    function transfer(address recipient, uint256 amount) public override returns (bool) {
        emit Transfer(msg.sender, recipient, amount); // doing something to prevent solc warning
        return false;
    }
}
