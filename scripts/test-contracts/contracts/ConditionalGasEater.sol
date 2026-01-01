// SPDX-License-Identifier: MIT
pragma solidity ^0.8.9;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";


/**
 * @title ConditionalGasEater
 * @dev ERC20 Token that conditionally wastes gas only when transferring to a specific router address
 * @notice Used for testing DCA extra gas handling in router trades
 */
contract ConditionalGasEater is ERC20 {
    address private immutable routerAddress;
    uint256 private immutable gasToWaste;

    /**
     * @param _routerAddress The address of the router pallet account (EVM H160)
     * @param _gasToWaste Amount of gas to waste on transfers to the router
     */
    constructor(address _routerAddress, uint256 _gasToWaste)
        ERC20("ConditionalGasEater", "CGEATER")
    {
        routerAddress = _routerAddress;
        gasToWaste = _gasToWaste;
        _mint(msg.sender, 1_000_000_000 * 10 ** decimals());
    }

    /**
     * @dev Wastes gas only if recipient is the router address
     * @param recipient The transfer recipient
     * @param amountToUse Amount of gas to waste
     */
    modifier wasteGasConditionally(address recipient, uint256 amountToUse) {
        if (recipient == routerAddress) {
            uint256 startGas = gasleft();
            uint256 counter = 0;
            while(startGas - gasleft() < amountToUse) {
                counter++;
            }
        }
        _;
    }

    /**
     * @dev Overrides transfer to conditionally waste gas
     */
    function transfer(address recipient, uint256 amount)
        public
        override
        wasteGasConditionally(recipient, gasToWaste)
        returns (bool)
    {
        return super.transfer(recipient, amount);
    }

    /**
     * @dev Overrides transferFrom to conditionally waste gas
     */
    function transferFrom(address sender, address recipient, uint256 amount)
        public
        override
        wasteGasConditionally(recipient, gasToWaste)
        returns (bool)
    {
        return super.transferFrom(sender, recipient, amount);
    }

    /**
     * @dev Getter for routerAddress (useful for testing)
     */
    function getRouterAddress() public view returns (address) {
        return routerAddress;
    }

    /**
     * @dev Getter for gasToWaste (useful for testing)
     */
    function getGasToWaste() public view returns (uint256) {
        return gasToWaste;
    }
}
