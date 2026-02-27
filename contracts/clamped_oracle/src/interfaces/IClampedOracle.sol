// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {AggregatorInterface} from "./AggregatorInterface.sol";

interface IClampedOracle is AggregatorInterface {
    event ClampedOracleInitialized(
        address indexed primaryFeed,
        address indexed secondaryFeed,
        uint256 maxDiffBps
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
