// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import "forge-std/Test.sol";

import {ClampedOracle} from "../src/ClampedOracle.sol";
import {IClampedOracle} from "../src/interfaces/IClampedOracle.sol";

import {MockAggregator} from "./mocks/MockAggregator.sol";
import {RevertingAggregator} from "./mocks/RevertingAggregator.sol";

import {MockHydraChainlinkOracle} from "./mocks/MockHydraChainlinkOracle.sol";
import {RevertingHydraChainlinkOracle} from "./mocks/RevertingHydraChainlinkOracle.sol";

contract ClampedOracleTest is Test {
    function p(
        uint256 whole,
        uint256 frac2Digits
    ) internal pure returns (int256) {
        return int256(whole * 1e8 + (frac2Digits * 1e6));
    }

    function testClampAboveBandExample() public {
        MockAggregator primary = new MockAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        primary.pushAnswer(p(1, 50), 100);
        secondary.pushAnswer(p(1, 0));

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestAnswer(), p(1, 10));
    }

    function testClampBelowBand() public {
        MockAggregator primary = new MockAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        primary.pushAnswer(p(0, 80), 100);
        secondary.pushAnswer(p(1, 0));

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestAnswer(), p(0, 90));
    }

    function testWithinBandReturnsPrimary() public {
        MockAggregator primary = new MockAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        primary.pushAnswer(p(1, 5), 100);
        secondary.pushAnswer(p(1, 0));

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestAnswer(), p(1, 5));
    }

    function testRevertOpenSecondaryRevertsReturnsPrimary() public {
        MockAggregator primary = new MockAggregator();
        RevertingHydraChainlinkOracle secondary = new RevertingHydraChainlinkOracle();

        primary.pushAnswer(p(1, 23), 100);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestAnswer(), p(1, 23));
    }

    function testRevertOpenPrimaryRevertsReturnsSecondary() public {
        RevertingAggregator primary = new RevertingAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        secondary.pushAnswer(p(0, 99));

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestAnswer(), p(0, 99));
    }

    function testBothFailRevertsNoValidPrice() public {
        RevertingAggregator primary = new RevertingAggregator();
        RevertingHydraChainlinkOracle secondary = new RevertingHydraChainlinkOracle();

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        vm.expectRevert(IClampedOracle.NoValidPrice.selector);
        oracle.latestAnswer();
    }

    function testLatestTimestampReturnsPrimaryTs() public {
        MockAggregator primary = new MockAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        primary.pushAnswer(p(1, 0), 777);
        secondary.pushAnswer(p(1, 0));

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestTimestamp(), 777);
    }

    function testLatestTimestampPrimaryRevertsNoValidPrice() public {
        RevertingAggregator primary = new RevertingAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        secondary.pushAnswer(p(1, 0));

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        vm.expectRevert(IClampedOracle.NoValidPrice.selector);
        oracle.latestTimestamp();
    }

    function testLatestTimestampPrimaryZeroNoValidPrice() public {
        MockAggregator primary = new MockAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        primary.pushAnswer(p(1, 0), 0);
        secondary.pushAnswer(p(1, 0));

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        vm.expectRevert(IClampedOracle.NoValidPrice.selector);
        oracle.latestTimestamp();
    }

    function testLatestRoundForwardsPrimaryRound() public {
        MockAggregator primary = new MockAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        primary.pushAnswer(p(1, 0), 100);
        uint256 r2 = primary.pushAnswer(p(1, 0), 101);

        secondary.pushAnswer(p(1, 0));

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestRound(), r2);
    }

    function testLatestRoundPrimaryReverts() public {
        RevertingAggregator primary = new RevertingAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        secondary.pushAnswer(p(1, 0));

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        vm.expectRevert();
        oracle.latestRound();
    }

    function testGetAnswerClampsUsingRoundId() public {
        MockAggregator primary = new MockAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        primary.setRoundData(10, p(1, 50), 111);
        secondary.setRoundData(10, p(1, 0));

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.getAnswer(10), p(1, 10));
    }

    function testGetTimestampUsesPrimaryRoundId() public {
        MockAggregator primary = new MockAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        primary.setRoundData(10, p(1, 0), 555);
        secondary.setRoundData(10, p(1, 0));

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.getTimestamp(10), 555);
    }

    function testGetTimestampPrimaryRevertsNoValidPrice() public {
        RevertingAggregator primary = new RevertingAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        secondary.setRoundData(10, p(1, 0));

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        vm.expectRevert(IClampedOracle.NoValidPrice.selector);
        oracle.getTimestamp(10);
    }

    function testConstructorZeroFeedReverts() public {
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        vm.expectRevert(IClampedOracle.InvalidFeed.selector);
        new ClampedOracle(address(0), address(secondary), 1000);
    }

    function testConstructorInvalidBpsReverts() public {
        MockAggregator primary = new MockAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        vm.expectRevert(IClampedOracle.InvalidBps.selector);
        new ClampedOracle(address(primary), address(secondary), 10_001);
    }

    function testPrimaryZeroAnswerFallsBackToSecondary() public {
        MockAggregator primary = new MockAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        primary.pushAnswer(int256(0), 100);
        secondary.pushAnswer(p(1, 0));

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestAnswer(), p(1, 0));
    }

    function testSecondaryZeroAnswerReturnsPrimaryNoClamp() public {
        MockAggregator primary = new MockAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        primary.pushAnswer(p(1, 23), 100);
        secondary.pushAnswer(int256(0));

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestAnswer(), p(1, 23));
    }

    function testDecimalsIsEight() public {
        MockAggregator primary = new MockAggregator();
        MockHydraChainlinkOracle secondary = new MockHydraChainlinkOracle();

        primary.pushAnswer(p(1, 0), 100);
        secondary.pushAnswer(p(1, 0));

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.decimals(), 8);
    }
}
