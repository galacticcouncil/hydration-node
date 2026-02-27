// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {IHydraChainlinkOracle} from "../../src/interfaces/IHydraChainlinkOracle.sol";

contract RevertingHydraChainlinkOracle is IHydraChainlinkOracle {
    function latestAnswer() external pure returns (int256) {
        revert("revert");
    }

    function getAnswer(uint256) external pure returns (int256) {
        revert("revert");
    }

    function decimals() external pure returns (uint8) {
        revert("revert");
    }
}
