// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {AggregatorInterface} from "../../src/interfaces/AggregatorInterface.sol";

contract RevertingAggregator is AggregatorInterface {
    function latestAnswer() external pure override returns (int256) {
        revert("revert");
    }

    function latestTimestamp() external pure override returns (uint256) {
        revert("revert");
    }

    function latestRound() external pure override returns (uint256) {
        revert("revert");
    }

    function getAnswer(uint256) external pure override returns (int256) {
        revert("revert");
    }

    function getTimestamp(uint256) external pure override returns (uint256) {
        revert("revert");
    }
}
