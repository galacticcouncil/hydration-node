// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface IHydraChainlinkOracle {
    function latestAnswer() external view returns (int256);

    function getAnswer(uint256 roundId) external view returns (int256);

    function decimals() external view returns (uint8);
}
