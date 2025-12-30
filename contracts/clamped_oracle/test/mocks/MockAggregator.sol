// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {IAggregatorV3} from "../../src/interfaces/IAggregatorV3.sol";

contract MockAggregatorV3 is IAggregatorV3 {
    uint8 private _decimals;
    uint256 private _version;

    int256 private _answer;
    uint80 private _roundId;
    uint80 private _answeredInRound;
    uint256 private _startedAt;
    uint256 private _updatedAt;

    constructor(uint8 decimals_) {
        _decimals = decimals_;
        _version = 1;
        _roundId = 1;
        _answeredInRound = 1;
        _startedAt = 1;
        _updatedAt = 1;
    }

    function setAnswer(int256 answer_, uint256 updatedAt_) external {
        _answer = answer_;
        _roundId += 1;
        _answeredInRound = _roundId;
        _startedAt = updatedAt_ > 0 ? updatedAt_ - 1 : 0;
        _updatedAt = updatedAt_;
    }

    function decimals() external view returns (uint8) {
        return _decimals;
    }

    function version() external view returns (uint256) {
        return _version;
    }

    function getRoundData(
        uint80
    ) external view returns (uint80, int256, uint256, uint256, uint80) {
        return latestRoundData();
    }

    function latestRoundData()
        public
        view
        returns (uint80, int256, uint256, uint256, uint80)
    {
        return (_roundId, _answer, _startedAt, _updatedAt, _answeredInRound);
    }
}
