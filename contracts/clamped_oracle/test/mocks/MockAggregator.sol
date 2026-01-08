// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {AggregatorInterface} from "../../src/interfaces/AggregatorInterface.sol";

contract MockAggregator is AggregatorInterface {
    struct Round {
        int256 answer;
        uint256 timestamp;
        bool exists;
    }

    mapping(uint256 => Round) private _rounds;
    uint256 private _latestRoundId;

    error NoDataPresent();

    constructor() {
        _latestRoundId = 1;
        _rounds[1] = Round({answer: 0, timestamp: 1, exists: true});
    }

    function pushAnswer(
        int256 answer_,
        uint256 timestamp_
    ) external returns (uint256 newRoundId) {
        newRoundId = _latestRoundId + 1;
        _latestRoundId = newRoundId;

        _rounds[newRoundId] = Round({
            answer: answer_,
            timestamp: timestamp_,
            exists: true
        });

        emit NewRound(newRoundId, msg.sender, timestamp_);
        emit AnswerUpdated(answer_, newRoundId, timestamp_);
    }

    function setRoundData(
        uint256 roundId_,
        int256 answer_,
        uint256 timestamp_
    ) external {
        _rounds[roundId_] = Round({
            answer: answer_,
            timestamp: timestamp_,
            exists: true
        });
        if (roundId_ > _latestRoundId) _latestRoundId = roundId_;

        emit NewRound(roundId_, msg.sender, timestamp_);
        emit AnswerUpdated(answer_, roundId_, timestamp_);
    }

    function latestAnswer() external view override returns (int256) {
        Round memory r = _rounds[_latestRoundId];
        if (!r.exists) revert NoDataPresent();
        return r.answer;
    }

    function latestTimestamp() external view override returns (uint256) {
        Round memory r = _rounds[_latestRoundId];
        if (!r.exists) revert NoDataPresent();
        return r.timestamp;
    }

    function latestRound() external view override returns (uint256) {
        return _latestRoundId;
    }

    function getAnswer(
        uint256 roundId
    ) external view override returns (int256) {
        Round memory r = _rounds[roundId];
        if (!r.exists) revert NoDataPresent();
        return r.answer;
    }

    function getTimestamp(
        uint256 roundId
    ) external view override returns (uint256) {
        Round memory r = _rounds[roundId];
        if (!r.exists) revert NoDataPresent();
        return r.timestamp;
    }
}
