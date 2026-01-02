// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import "forge-std/Test.sol";

import {ClampedOracle} from "../src/ClampedOracle.sol";
import {IClampedOracle} from "../src/interfaces/IClampedOracle.sol";
import {MockAggregatorV3} from "./mocks/MockAggregator.sol";
import {RevertingAggregatorV3} from "./mocks/RevertingAggregator.sol";

contract ClampedOracleTest is Test {
    uint8 constant DECIMALS = 8;

    function p(uint256 whole, uint256 frac) internal pure returns (int256) {
        return int256(whole * 1e8 + (frac * 1e6));
    }

    function testClampAboveBandExample() public {
        MockAggregatorV3 primary = new MockAggregatorV3(DECIMALS);
        MockAggregatorV3 secondary = new MockAggregatorV3(DECIMALS);

        primary.setAnswer(p(1, 50), 100);
        secondary.setAnswer(p(1, 0), 100);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        (, int256 ans, , , ) = oracle.latestRoundData();
        assertEq(ans, p(1, 10));
    }

    function testClampBelowBand() public {
        MockAggregatorV3 primary = new MockAggregatorV3(DECIMALS);
        MockAggregatorV3 secondary = new MockAggregatorV3(DECIMALS);

        primary.setAnswer(p(0, 80), 100);
        secondary.setAnswer(p(1, 0), 100);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        (, int256 ans, , , ) = oracle.latestRoundData();
        assertEq(ans, p(0, 90));
    }

    function testWithinBandReturnsPrimary() public {
        MockAggregatorV3 primary = new MockAggregatorV3(DECIMALS);
        MockAggregatorV3 secondary = new MockAggregatorV3(DECIMALS);

        primary.setAnswer(p(1, 5), 100);
        secondary.setAnswer(p(1, 0), 100);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        (, int256 ans, , , ) = oracle.latestRoundData();
        assertEq(ans, p(1, 5));
    }

    function testDecimalsMismatchReverts() public {
        MockAggregatorV3 primary = new MockAggregatorV3(8);
        MockAggregatorV3 secondary = new MockAggregatorV3(18);

        vm.expectRevert(IClampedOracle.DecimalsMismatch.selector);
        new ClampedOracle(address(primary), address(secondary), 1000);
    }

    function testRevertOpenSecondaryFailsReturnsPrimary() public {
        MockAggregatorV3 primary = new MockAggregatorV3(DECIMALS);
        RevertingAggregatorV3 secondary = new RevertingAggregatorV3(DECIMALS);

        primary.setAnswer(p(1, 23), 100);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        (, int256 ans, , , ) = oracle.latestRoundData();
        assertEq(ans, p(1, 23));
    }

    function testRevertOpenPrimaryFailsReturnsSecondary() public {
        RevertingAggregatorV3 primary = new RevertingAggregatorV3(DECIMALS);
        MockAggregatorV3 secondary = new MockAggregatorV3(DECIMALS);

        secondary.setAnswer(p(0, 99), 100);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        (, int256 ans, , , ) = oracle.latestRoundData();
        assertEq(ans, p(0, 99));
    }

    function testBothFailRevertsNoValidPrice() public {
        RevertingAggregatorV3 primary = new RevertingAggregatorV3(DECIMALS);
        RevertingAggregatorV3 secondary = new RevertingAggregatorV3(DECIMALS);

        ClampedOracle oracle = new ClampedOracle(
            address(primary),
            address(secondary),
            1000
        );

        vm.expectRevert(IClampedOracle.NoValidPrice.selector);
        oracle.latestRoundData();
    }

    function testConstructorZeroFeedReverts() public {
        MockAggregatorV3 secondary = new MockAggregatorV3(DECIMALS);

        vm.expectRevert(IClampedOracle.InvalidFeed.selector);
        new ClampedOracle(address(0), address(secondary), 1000);
    }

    function testConstructorInvalidBpsReverts() public {
        MockAggregatorV3 primary = new MockAggregatorV3(DECIMALS);
        MockAggregatorV3 secondary = new MockAggregatorV3(DECIMALS);

        vm.expectRevert(IClampedOracle.InvalidBps.selector);
        new ClampedOracle(address(primary), address(secondary), 10_001);
    }
}
