// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {IAggregatorV3} from "./IAggregatorV3.sol";

interface IClampedOracle is IAggregatorV3 {
    event ClampedOracleInitialized(
        address indexed primaryFeed,
        address indexed secondaryFeed,
        uint256 maxDiffBps,
        uint8 decimals
    );

    event ClampParamsUpdated(
        address indexed primaryFeed,
        address indexed secondaryFeed,
        uint256 maxDiffBps
    );

    error InvalidFeed();
    error InvalidBps();
    error DecimalsMismatch();
    error NoValidPrice();

    function primary() external view returns (address);

    function secondary() external view returns (address);

    function maxDiffBps() external view returns (uint256);
}
