// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {IClampedOracle} from "./interfaces/IClampedOracle.sol";
import {AggregatorInterface} from "./interfaces/AggregatorInterface.sol";
import {IHydraChainlinkOracle} from "./interfaces/IHydraChainlinkOracle.sol";

contract ClampedOracle is IClampedOracle {
    uint256 public constant MAX_BPS = 10_000;

    AggregatorInterface private immutable primaryAgg;
    IHydraChainlinkOracle private immutable secondaryAgg;

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
        secondaryAgg = IHydraChainlinkOracle(secondaryFeed);
        maxDiffBps = maxDiffBps_;

        emit ClampedOracleInitialized(primaryFeed, secondaryFeed, maxDiffBps_);
    }

    function primary() external view override returns (address) {
        return address(primaryAgg);
    }

    function secondary() external view override returns (address) {
        return address(secondaryAgg);
    }

    function decimals() external pure returns (uint8) {
        return 8;
    }

    function latestAnswer() public view override returns (int256) {
        (bool pOk, int256 pAns) = _tryLatestAnswerPrimary();
        (bool sOk, int256 sAns) = _tryLatestAnswerSecondary();

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
        (bool pOk, uint256 pTs) = _tryLatestTimestampPrimary();

        if (pOk) return pTs;
        revert NoValidPrice();
    }

    function latestRound() external view override returns (uint256) {
        return primaryAgg.latestRound();
    }

    function getAnswer(
        uint256 roundId
    ) external view override returns (int256) {
        (bool pOk, int256 pAns) = _tryGetAnswerPrimary(roundId);
        (bool sOk, int256 sAns) = _tryGetAnswerSecondary(roundId);

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

    function getTimestamp(
        uint256 roundId
    ) external view override returns (uint256) {
        (bool pOk, uint256 pTs) = _tryGetTimestampPrimary(roundId);

        if (pOk) return pTs;
        revert NoValidPrice();
    }

    function _tryLatestAnswerPrimary()
        internal
        view
        returns (bool ok, int256 ans)
    {
        try primaryAgg.latestAnswer() returns (int256 a) {
            if (a <= 0) return (false, 0);
            return (true, a);
        } catch {
            return (false, 0);
        }
    }

    function _tryLatestAnswerSecondary()
        internal
        view
        returns (bool ok, int256 ans)
    {
        try secondaryAgg.latestAnswer() returns (int256 a) {
            if (a <= 0) return (false, 0);
            return (true, a);
        } catch {
            return (false, 0);
        }
    }

    function _tryLatestTimestampPrimary()
        internal
        view
        returns (bool ok, uint256 ts)
    {
        try primaryAgg.latestTimestamp() returns (uint256 t) {
            if (t == 0) return (false, 0);
            return (true, t);
        } catch {
            return (false, 0);
        }
    }

    function _tryGetAnswerPrimary(
        uint256 roundId
    ) internal view returns (bool ok, int256 ans) {
        try primaryAgg.getAnswer(roundId) returns (int256 a) {
            if (a <= 0) return (false, 0);
            return (true, a);
        } catch {
            return (false, 0);
        }
    }

    function _tryGetAnswerSecondary(
        uint256 roundId
    ) internal view returns (bool ok, int256 ans) {
        try secondaryAgg.getAnswer(roundId) returns (int256 a) {
            if (a <= 0) return (false, 0);
            return (true, a);
        } catch {
            return (false, 0);
        }
    }

    function _tryGetTimestampPrimary(
        uint256 roundId
    ) internal view returns (bool ok, uint256 ts) {
        try primaryAgg.getTimestamp(roundId) returns (uint256 t) {
            if (t == 0) return (false, 0);
            return (true, t);
        } catch {
            return (false, 0);
        }
    }
}
