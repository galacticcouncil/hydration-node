// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract HydraToken is ERC20 {
    constructor() ERC20("Hydra", "HYDRA") {
        _mint(msg.sender, 1_000_000_000 * 10 ** decimals());
    }
}
