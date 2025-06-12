// SPDX-License-Identifier: MIT
pragma solidity ^0.8.9;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";


/**
 * @title GasEatingToken
 * @dev ERC20 Token with intentional gas consumption on transfers
 */
contract GasEatingToken is ERC20 {
   constructor() ERC20("GasEater", "EATER") {
        _mint(msg.sender, 1_000_000_000 * 10 ** decimals());
    }

    /**
     * @dev Wastes gas until a specific threshold is reached
     * @param amountToUse Amount of has to waste
     */
    modifier wasteEther(uint256 amountToUse) {
        uint256 startGas = gasleft();
        uint256 counter = 0;
        while(startGas - gasleft() < amountToUse) {
            counter++;
        }
        _;
    }

    /**
     * @dev Overrides the transfer function to waste gas
     */
    function transfer(address recipient, uint256 amount)
        public
        override
        wasteEther(380000)
        returns (bool)
    {
        return super.transfer(recipient, amount);
    }

    /**
     * @dev Overrides the transferFrom function to waste gas
     */
    function transferFrom(address sender, address recipient, uint256 amount)
        public
        override
        wasteEther(380000)
        returns (bool)
    {
        return super.transferFrom(sender, recipient, amount);
    }
}