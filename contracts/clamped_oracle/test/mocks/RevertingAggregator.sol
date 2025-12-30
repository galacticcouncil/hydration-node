// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {IAggregatorV3} from "../../src/interfaces/IAggregatorV3.sol";

contract RevertingAggregatorV3 is IAggregatorV3 {
    uint8 private _decimals;

    constructor(uint8 decimals_) {
        _decimals = decimals_;
    }

    function decimals() external view returns (uint8) {
        return _decimals;
    }

    function description() external pure returns (string memory) {
        return "reverting";
    }

    function version() external pure returns (uint256) {
        return 1;
    }

    function getRoundData(
        uint80
    ) external pure returns (uint80, int256, uint256, uint256, uint80) {
        revert("revert");
    }

    function latestRoundData()
        external
        pure
        returns (uint80, int256, uint256, uint256, uint80)
    {
        revert("revert");
    }
}
