// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {IHydraChainlinkOracle} from "../../src/interfaces/IHydraChainlinkOracle.sol";

contract MockHydraChainlinkOracle is IHydraChainlinkOracle {
    uint8 private immutable _decimals;

    uint256 public latestRoundId;
    mapping(uint256 => int256) public answerOf;

    constructor() {
        _decimals = 8;
    }

    function decimals() external view returns (uint8) {
        return _decimals;
    }

    function latestAnswer() external view returns (int256) {
        return answerOf[latestRoundId];
    }

    function getAnswer(uint256 roundId) external view returns (int256) {
        return answerOf[roundId];
    }

    function pushAnswer(int256 ans) external returns (uint256 roundId) {
        latestRoundId += 1;
        roundId = latestRoundId;
        answerOf[roundId] = ans;
    }

    function setRoundData(uint256 roundId, int256 ans) external {
        answerOf[roundId] = ans;
        if (roundId > latestRoundId) latestRoundId = roundId;
    }
}
