// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import "forge-std/Test.sol";

import {ClampedOracle} from "../src/ClampedOracle.sol";
import {IClampedOracle} from "../src/interfaces/IClampedOracle.sol";
import {MockAggregator} from "./mocks/MockAggregator.sol";
import {RevertingAggregator} from "./mocks/RevertingAggregator.sol";

contract ClampedOracleTest is Test {
    function p(
        uint256 whole,
        uint256 frac2Digits
    ) internal pure returns (int256) {
        return int256(whole * 1e8 + (frac2Digits * 1e6));
    }

    function testClampAboveBandExample() public {
        MockAggregator primary = new MockAggregator();
        MockAggregator secondary = new MockAggregator();

        primary.pushAnswer(p(1, 50), 100);
        secondary.pushAnswer(p(1, 0), 100);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        int256 ans = oracle.latestAnswer();
        assertEq(ans, p(1, 10));
    }

    function testClampBelowBand() public {
        MockAggregator primary = new MockAggregator();
        MockAggregator secondary = new MockAggregator();

        primary.pushAnswer(p(0, 80), 100);
        secondary.pushAnswer(p(1, 0), 100);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        int256 ans = oracle.latestAnswer();
        assertEq(ans, p(0, 90));
    }

    function testWithinBandReturnsPrimary() public {
        MockAggregator primary = new MockAggregator();
        MockAggregator secondary = new MockAggregator();

        primary.pushAnswer(p(1, 5), 100);
        secondary.pushAnswer(p(1, 0), 100);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        int256 ans = oracle.latestAnswer();
        assertEq(ans, p(1, 5));
    }

    function testRevertOpenSecondaryRevertsReturnsPrimary() public {
        MockAggregator primary = new MockAggregator();
        RevertingAggregator secondary = new RevertingAggregator();

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
        MockAggregator secondary = new MockAggregator();

        secondary.pushAnswer(p(0, 99), 100);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestAnswer(), p(0, 99));
    }

    function testBothFailRevertsNoValidPrice() public {
        RevertingAggregator primary = new RevertingAggregator();
        RevertingAggregator secondary = new RevertingAggregator();

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        vm.expectRevert(IClampedOracle.NoValidPrice.selector);
        oracle.latestAnswer();
    }

    function testLatestTimestampReturnsMinWhenBothOk() public {
        MockAggregator primary = new MockAggregator();
        MockAggregator secondary = new MockAggregator();

        primary.pushAnswer(p(1, 0), 200);
        secondary.pushAnswer(p(1, 0), 150);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestTimestamp(), 150);
    }

    function testLatestTimestampSecondaryFailsReturnsPrimaryTs() public {
        MockAggregator primary = new MockAggregator();
        RevertingAggregator secondary = new RevertingAggregator();

        primary.pushAnswer(p(1, 0), 777);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestTimestamp(), 777);
    }

    function testLatestRoundForwardsPrimaryRound() public {
        MockAggregator primary = new MockAggregator();
        MockAggregator secondary = new MockAggregator();

        primary.pushAnswer(p(1, 0), 100);
        uint256 r2 = primary.pushAnswer(p(1, 0), 101);

        secondary.pushAnswer(p(1, 0), 100);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestRound(), r2);
    }

    function testLatestRoundFallsBackToSecondaryIfPrimaryReverts() public {
        RevertingAggregator primary = new RevertingAggregator();
        MockAggregator secondary = new MockAggregator();

        uint256 r = secondary.pushAnswer(p(1, 0), 100);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestRound(), r);
    }

    function testGetAnswerAndGetTimestampReturnLatest() public {
        MockAggregator primary = new MockAggregator();
        MockAggregator secondary = new MockAggregator();

        primary.setRoundData(10, p(9, 99), 10);
        secondary.setRoundData(10, p(1, 0), 10);

        primary.pushAnswer(p(1, 5), 200);
        secondary.pushAnswer(p(1, 0), 150);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.getAnswer(10), oracle.latestAnswer());
        assertEq(oracle.getTimestamp(10), oracle.latestTimestamp());
    }

    function testConstructorZeroFeedReverts() public {
        MockAggregator secondary = new MockAggregator();

        vm.expectRevert(IClampedOracle.InvalidFeed.selector);
        new ClampedOracle(address(0), address(secondary), 1000);
    }

    function testConstructorInvalidBpsReverts() public {
        MockAggregator primary = new MockAggregator();
        MockAggregator secondary = new MockAggregator();

        vm.expectRevert(IClampedOracle.InvalidBps.selector);
        new ClampedOracle(address(primary), address(secondary), 10_001);
    }

    function testPrimaryZeroAnswerFallsBackToSecondary() public {
        MockAggregator primary = new MockAggregator();
        MockAggregator secondary = new MockAggregator();

        primary.pushAnswer(int256(0), 100);
        secondary.pushAnswer(p(1, 0), 100);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestAnswer(), p(1, 0));
    }

    function testSecondaryZeroAnswerReturnsPrimaryNoClamp() public {
        MockAggregator primary = new MockAggregator();
        MockAggregator secondary = new MockAggregator();

        primary.pushAnswer(p(1, 23), 100);
        secondary.pushAnswer(int256(0), 100);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestAnswer(), p(1, 23));
    }

    function testLatestTimestampZeroFallsBack() public {
        MockAggregator primary = new MockAggregator();
        MockAggregator secondary = new MockAggregator();

        primary.pushAnswer(p(1, 0), 0);
        secondary.pushAnswer(p(1, 0), 123);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        assertEq(oracle.latestTimestamp(), 123);
    }
}
