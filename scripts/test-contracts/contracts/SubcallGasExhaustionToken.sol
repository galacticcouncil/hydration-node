// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/**
 * @title GasWaster
 * @dev Helper contract that wastes gas in a function call
 * This is deployed as a separate contract to force EIP-150's 63/64 gas rule
 */
contract GasWaster {
    uint256 public counter;

    /// @notice Wastes approximately gasToWaste amount of gas
    /// @param gasToWaste The amount of gas to consume
    /// @return success Always returns true if it completes
    function wasteGas(uint256 gasToWaste) external returns (bool) {
        uint256 gasStart = gasleft();

        // Waste gas by incrementing a storage variable in a loop
        // This will consume gas until we've used approximately gasToWaste
        // Use subtraction that won't underflow: check consumed gas vs target
        while (gasStart - gasleft() < gasToWaste && gasleft() > 0) {
            counter++;
        }

        return true;
    }
}

/**
 * @title SubcallGasExhaustionToken
 * @dev ERC20 token that makes subcalls to waste gas, mimicking AAVE's liquidation behavior
 *
 * This contract reproduces the scenario where:
 * 1. Main contract (this token) is called during a transfer
 * 2. It makes a subcall to GasWaster contract
 * 3. Due to EIP-150's 63/64 rule, the subcall only gets 63/64 of remaining gas
 * 4. If the subcall runs out of gas, it returns false (not a revert)
 * 5. This contract detects the failure and explicitly reverts with no message
 * 6. This mimics AAVE's liquidationCall behavior
 */
contract SubcallGasExhaustionToken is ERC20 {
    GasWaster public gasWaster;
    address public routerAddress;
    uint256 public gasToWaste;
    bool public enableGasWasting;

    event GasWastingEnabled(bool enabled);
    event SubcallFailed(uint256 gasLeft);

    constructor(
        address _routerAddress,
        uint256 _gasToWaste
    ) ERC20("SubcallGasToken", "SGT") {
        routerAddress = _routerAddress;
        gasToWaste = _gasToWaste;
        enableGasWasting = true;

        // Deploy GasWaster helper contract during construction
        // This creates a separate contract that will be called via external call
        gasWaster = new GasWaster();

        // Mint initial supply to deployer
        _mint(msg.sender, 1000000 * 10**decimals());
    }

    /// @notice Override transfer to add conditional subcall gas wasting
    /// @dev Only wastes gas when transferring to the router address
    function transfer(address to, uint256 amount) public override returns (bool) {
        // Only waste gas when transferring to router (mimics AAVE liquidation scenario)
        if (enableGasWasting && to == routerAddress) {
            // Make external call to waste gas
            // This gets 63/64 of remaining gas per EIP-150
            // If the subcall runs out of gas, it returns false instead of reverting
            bool success = gasWaster.wasteGas(gasToWaste);

            if (!success) {
                // If subcall failed (due to gas exhaustion), revert with no message
                // This mimics AAVE's behavior when a subcall fails
                emit SubcallFailed(gasleft());
                revert();
            }
        }

        return super.transfer(to, amount);
    }

    /// @notice Override transferFrom to add conditional subcall gas wasting
    /// @dev Only wastes gas when transferring to the router address
    function transferFrom(address from, address to, uint256 amount) public override returns (bool) {
        // Only waste gas when transferring to router
        if (enableGasWasting && to == routerAddress) {
            // Make external call to waste gas (gets 63/64 of gas per EIP-150)
            bool success = gasWaster.wasteGas(gasToWaste);

            if (!success) {
                // Revert with no message - mimics AAVE liquidation revert
                emit SubcallFailed(gasleft());
                revert();
            }
        }

        return super.transferFrom(from, to, amount);
    }

    /// @notice Enable or disable gas wasting for testing
    /// @dev This allows testing successful transfers after fixing gas issues
    function setEnableGasWasting(bool _enable) external {
        enableGasWasting = _enable;
        emit GasWastingEnabled(_enable);
    }

    /// @notice Get the address of the GasWaster helper contract
    function getGasWasterAddress() external view returns (address) {
        return address(gasWaster);
    }
}
