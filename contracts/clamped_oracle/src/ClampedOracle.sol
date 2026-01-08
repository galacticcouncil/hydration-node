// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {IClampedOracle} from "./interfaces/IClampedOracle.sol";
import {AggregatorInterface} from "./interfaces/AggregatorInterface.sol";

contract ClampedOracle is IClampedOracle {
    uint256 public constant MAX_BPS = 10_000;

    AggregatorInterface private immutable primaryAgg;
    AggregatorInterface private immutable secondaryAgg;

    uint256 public immutable override maxDiffBps;

    constructor(
        address primaryFeed,
        address secondaryFeed,
        uint256 maxDiffBps_
    ) {
        if (primaryFeed == address(0) || secondaryFeed == address(0))
            revert InvalidFeed();
        if (maxDiffBps_ > MAX_BPS) revert InvalidBps();

        primaryAgg = AggregatorInterface(primaryFeed);
        secondaryAgg = AggregatorInterface(secondaryFeed);
        maxDiffBps = maxDiffBps_;

        emit ClampedOracleInitialized(primaryFeed, secondaryFeed, maxDiffBps_);
    }

    function primary() external view override returns (address) {
        return address(primaryAgg);
    }

    function secondary() external view override returns (address) {
        return address(secondaryAgg);
    }

    function latestAnswer() public view override returns (int256) {
        (bool pOk, int256 pAns) = _tryLatestAnswer(primaryAgg);
        (bool sOk, int256 sAns) = _tryLatestAnswer(secondaryAgg);

        if (!pOk && !sOk) revert NoValidPrice();
        if (!sOk) return pAns;
        if (!pOk) return sAns;

        if (pAns <= 0 || sAns <= 0) revert NoValidPrice();

        uint256 P = uint256(pAns);
        uint256 S = uint256(sAns);

        uint256 lower = (S * (MAX_BPS - maxDiffBps)) / MAX_BPS;
        uint256 upper = (S * (MAX_BPS + maxDiffBps)) / MAX_BPS;

        uint256 out = P;
        if (out < lower) out = lower;
        if (out > upper) out = upper;

        return int256(out);
    }

    function latestTimestamp() public view override returns (uint256) {
        (bool pOk, uint256 pTs) = _tryLatestTimestamp(primaryAgg);
        (bool sOk, uint256 sTs) = _tryLatestTimestamp(secondaryAgg);

        if (!pOk && !sOk) revert NoValidPrice();
        if (!sOk) return pTs;
        if (!pOk) return sTs;

        return pTs < sTs ? pTs : sTs;
    }

    function latestRound() external view override returns (uint256) {
        try primaryAgg.latestRound() returns (uint256 r) {
            return r;
        } catch {
            return secondaryAgg.latestRound();
        }
    }

    function getAnswer(uint256) external view override returns (int256) {
        return latestAnswer();
    }

    function getTimestamp(uint256) external view override returns (uint256) {
        return latestTimestamp();
    }

    function _tryLatestAnswer(
        AggregatorInterface agg
    ) internal view returns (bool ok, int256 ans) {
        try agg.latestAnswer() returns (int256 a) {
            if (a <= 0) return (false, 0);
            return (true, a);
        } catch {
            return (false, 0);
        }
    }

    function _tryLatestTimestamp(
        AggregatorInterface agg
    ) internal view returns (bool ok, uint256 ts) {
        try agg.latestTimestamp() returns (uint256 t) {
            if (t == 0) return (false, 0);
            return (true, t);
        } catch {
            return (false, 0);
        }
    }
}
