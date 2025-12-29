// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {IClampedOracle} from "./interfaces/IClampedOracle.sol";
import {IAggregatorV3} from "./interfaces/IAggregatorV3.sol";

contract ClampedOracle is IClampedOracle {
    uint256 public constant MAX_BPS = 10_000;

    IAggregatorV3 private immutable primaryAgg;
    IAggregatorV3 private immutable secondaryAgg;

    uint256 public immutable override maxDiffBps;

    uint8 private immutable _decimals;
    string private _description;

    constructor(
        address primaryFeed,
        address secondaryFeed,
        uint256 maxDiffBps_,
        string memory description_
    ) {
        if (primaryFeed == address(0) || secondaryFeed == address(0))
            revert InvalidFeed();
        if (maxDiffBps_ > MAX_BPS) revert InvalidBps();

        IAggregatorV3 p = IAggregatorV3(primaryFeed);
        IAggregatorV3 s = IAggregatorV3(secondaryFeed);

        uint8 pd = p.decimals();
        uint8 sd = s.decimals();
        if (pd != sd) revert DecimalsMismatch();

        primaryAgg = p;
        secondaryAgg = s;
        maxDiffBps = maxDiffBps_;

        _decimals = pd;
        _description = description_;

        emit ClampedOracleInitialized(
            primaryFeed,
            secondaryFeed,
            maxDiffBps_,
            pd,
            description_
        );
    }

    function primary() external view override returns (address) {
        return address(primaryAgg);
    }

    function secondary() external view override returns (address) {
        return address(secondaryAgg);
    }

    function decimals() external view override returns (uint8) {
        return _decimals;
    }

    function description() external view override returns (string memory) {
        return _description;
    }

    function version() external pure override returns (uint256) {
        return 1;
    }

    function getRoundData(
        uint80
    )
        external
        view
        override
        returns (
            uint80 roundId,
            int256 answer,
            uint256 startedAt,
            uint256 updatedAt,
            uint80 answeredInRound
        )
    {
        return latestRoundData();
    }

    function latestRoundData()
        public
        view
        override
        returns (
            uint80 roundId,
            int256 answer,
            uint256 startedAt,
            uint256 updatedAt,
            uint80 answeredInRound
        )
    {
        (
            bool pOk,
            uint80 pRound,
            int256 pAns,
            uint256 pStart,
            uint256 pUpdate,
            uint80 pAnswered
        ) = _tryLatest(primaryAgg);

        (
            bool sOk,
            uint80 sRound,
            int256 sAns,
            uint256 sStart,
            uint256 sUpdate,
            uint80 sAnswered
        ) = _tryLatest(secondaryAgg);

        if (!pOk && !sOk) revert NoValidPrice();

        if (!sOk) {
            return (pRound, pAns, pStart, pUpdate, pAnswered);
        }
        if (!pOk) {
            return (sRound, sAns, sStart, sUpdate, sAnswered);
        }

        if (pAns <= 0 || sAns <= 0) revert NoValidPrice();

        uint256 P = uint256(pAns);
        uint256 S = uint256(sAns);

        uint256 lower = (S * (MAX_BPS - maxDiffBps)) / MAX_BPS;
        uint256 upper = (S * (MAX_BPS + maxDiffBps)) / MAX_BPS;

        uint256 out = P;
        if (out < lower) out = lower;
        if (out > upper) out = upper;

        uint256 outStart = pStart < sStart ? pStart : sStart;
        uint256 outUpdate = pUpdate < sUpdate ? pUpdate : sUpdate;

        return (pRound, int256(out), outStart, outUpdate, pAnswered);
    }

    function _tryLatest(
        IAggregatorV3 agg
    )
        internal
        view
        returns (
            bool ok,
            uint80 roundId,
            int256 ans,
            uint256 startedAt,
            uint256 updatedAt,
            uint80 answeredInRound
        )
    {
        try agg.latestRoundData() returns (
            uint80 r,
            int256 a,
            uint256 s,
            uint256 u,
            uint80 ar
        ) {
            if (a <= 0 || u == 0) {
                return (false, 0, 0, 0, 0, 0);
            }
            return (true, r, a, s, u, ar);
        } catch {
            return (false, 0, 0, 0, 0, 0);
        }
    }
}
