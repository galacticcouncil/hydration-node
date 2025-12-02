// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import "forge-std/Test.sol";

import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";
import {GasFaucet} from "../src/GasFaucet.sol";
import {IGasFaucet} from "../src/interfaces/IGasFaucet.sol";
import {GasVoucher} from "../src/GasVoucher.sol";
import {IGasVoucher} from "../src/interfaces/IGasVoucher.sol";
import "../src/utils/Errors.sol";

contract FaucetTest is Test {
    GasFaucet faucet;
    GasVoucher voucher;

    address owner = address(0xABCD);
    address mpc = address(0xBEEF);
    address alice = address(0xA11CE);
    address bob = address(0xB0B);

    uint256 constant THRESH = 1 ether;

    function setUp() public {
        vm.deal(owner, 100 ether);

        vm.startPrank(owner);
        voucher = new GasVoucher(owner);
        faucet = new GasFaucet(mpc, address(voucher), THRESH, owner);
        voucher.setFaucet(address(faucet));
        vm.stopPrank();
    }

    function test_constructor_initializes() public {
        assertEq(faucet.owner(), owner);
        assertEq(faucet.mpc(), mpc);
        assertEq(address(faucet.voucher()), address(voucher));
        assertEq(faucet.minEthThreshold(), THRESH);
    }

    function test_onlyOwner_can_setMPC() public {
        vm.prank(alice);
        vm.expectRevert(
            abi.encodeWithSelector(
                Ownable.OwnableUnauthorizedAccount.selector,
                alice
            )
        );
        faucet.setMPC(bob);

        vm.prank(owner);
        vm.expectRevert(ZeroAddress.selector);
        faucet.setMPC(address(0));

        vm.prank(owner);
        vm.expectEmit(true, true, true, true);
        emit IGasFaucet.MPCUpdated(alice);
        faucet.setMPC(alice);
        assertEq(faucet.mpc(), alice);
    }

    function test_onlyOwner_can_setMinEthThreshold() public {
        vm.prank(alice);
        vm.expectRevert(
            abi.encodeWithSelector(
                Ownable.OwnableUnauthorizedAccount.selector,
                alice
            )
        );
        faucet.setMinEthThreshold(2 ether);

        vm.prank(owner);
        vm.expectEmit(true, true, true, true);
        emit IGasFaucet.ThresholdUpdated(2 ether);
        faucet.setMinEthThreshold(2 ether);
        assertEq(faucet.minEthThreshold(), 2 ether);
    }

    function test_fund_sends_eth_when_balance_sufficient_and_above_threshold()
        public
    {
        vm.deal(address(faucet), 5 ether);

        uint256 amount = 1 ether;
        uint256 aliceBefore = alice.balance;

        vm.prank(mpc);
        vm.expectEmit(true, true, true, true);
        emit IGasFaucet.Funded(alice, amount);
        faucet.fund(alice, amount);

        assertEq(
            alice.balance,
            aliceBefore + amount,
            "ETH transferred to recipient"
        );
        assertEq(voucher.balanceOf(alice), 0);
    }

    function test_fund_mints_voucher_when_balance_below_threshold() public {
        vm.deal(address(faucet), 0.5 ether);
        uint256 amount = 0.7 ether;

        vm.prank(mpc);
        vm.expectEmit(true, true, true, true);
        emit IGasFaucet.VoucherIssued(alice, amount);
        faucet.fund(alice, amount);

        assertEq(alice.balance, 0, "no ETH sent");
        assertEq(voucher.balanceOf(alice), amount, "voucher minted");
    }

    function test_fund_mints_voucher_when_balance_insufficient_for_amount()
        public
    {
        vm.deal(address(faucet), 1.2 ether);
        uint256 amount = 2 ether;
        vm.prank(mpc);
        vm.expectEmit(true, true, true, true);
        emit IGasFaucet.VoucherIssued(alice, amount);
        faucet.fund(alice, amount);

        assertEq(alice.balance, 0);
        assertEq(voucher.balanceOf(alice), amount);
    }

    function test_fund_reverts_on_zero_to() public {
        vm.deal(address(faucet), 2 ether);
        vm.prank(mpc);
        vm.expectRevert(ZeroAddress.selector);
        faucet.fund(address(0), 1 ether);
    }

    function test_fund_only_mpc() public {
        vm.deal(address(faucet), 2 ether);
        vm.prank(alice);
        vm.expectRevert(NotMPC.selector);
        faucet.fund(bob, 0.1 ether);
    }

    function test_redeem_happy_path() public {
        vm.deal(address(faucet), 0.2 ether);
        uint256 amount = 0.5 ether;

        vm.prank(mpc);
        faucet.fund(alice, amount);
        assertEq(voucher.balanceOf(alice), amount);

        vm.deal(address(faucet), 10 ether);

        uint256 before = alice.balance;
        vm.prank(alice);
        vm.expectEmit(true, true, true, true);
        emit IGasFaucet.Redeemed(alice, amount);
        faucet.redeem(amount);

        assertEq(alice.balance, before + amount, "redeemed ETH");
        assertEq(voucher.balanceOf(alice), 0, "voucher burned");
    }

    function test_redeem_reverts_when_faucet_low() public {
        vm.deal(address(faucet), 0.1 ether);
        vm.prank(mpc);
        faucet.fund(alice, 0.4 ether);
        assertEq(voucher.balanceOf(alice), 0.4 ether);

        vm.deal(address(faucet), 0.2 ether);
        vm.prank(alice);
        vm.expectRevert(FaucetLowBalance.selector);
        faucet.redeem(0.4 ether);
    }

    function test_redeem_reverts_zero_amount() public {
        vm.prank(alice);
        vm.expectRevert(ZeroAmount.selector);
        faucet.redeem(0);
    }

    function test_withdraw_only_owner() public {
        vm.deal(address(faucet), 5 ether);

        // Non-owner
        vm.prank(alice);
        vm.expectRevert(
            abi.encodeWithSelector(
                Ownable.OwnableUnauthorizedAccount.selector,
                alice
            )
        );
        faucet.withdraw(payable(bob), 1 ether);

        // Owner ok
        uint256 before = bob.balance;
        vm.prank(owner);
        vm.expectEmit(true, true, true, true);
        emit IGasFaucet.Withdrawn(bob, 1 ether);
        faucet.withdraw(payable(bob), 1 ether);

        assertEq(bob.balance, before + 1 ether);
    }

    function test_setVoucher_only_owner_and_non_zero() public {
        address newVoucher = address(0xC0FFEE);

        // non-owner
        vm.prank(alice);
        vm.expectRevert(
            abi.encodeWithSelector(
                Ownable.OwnableUnauthorizedAccount.selector,
                alice
            )
        );
        faucet.setVoucher(newVoucher);

        // zero addr
        vm.prank(owner);
        vm.expectRevert(ZeroAddress.selector);
        faucet.setVoucher(address(0));

        // happy path
        vm.prank(owner);
        faucet.setVoucher(newVoucher);
        assertEq(address(faucet.voucher()), newVoucher);
    }

    function test_redeem_more_than_voucher_balance_reverts() public {
        vm.deal(address(faucet), 0.1 ether);

        vm.prank(mpc);
        faucet.fund(alice, 1 ether);
        assertEq(voucher.balanceOf(alice), 1 ether);

        vm.prank(alice);
        vm.expectRevert(FaucetLowBalance.selector);
        faucet.redeem(1 ether);

        vm.deal(address(faucet), 10 ether);

        vm.prank(alice);
        vm.expectRevert();
        faucet.redeem(2 ether);

        uint256 aliceVoucherBalBefore = voucher.balanceOf(alice);
        uint256 aliceEthBalBefore = alice.balance;
        assertEq(aliceEthBalBefore, 0 ether);

        vm.prank(alice);
        faucet.redeem(1 ether);

        uint256 aliceVoucherBalAfter = voucher.balanceOf(alice);
        uint256 aliceEthBalAfter = alice.balance;

        assertEq(aliceVoucherBalBefore - aliceVoucherBalAfter, 1 ether);
        assertEq(aliceEthBalAfter - aliceEthBalBefore, 1 ether);
    }
}
