// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import "forge-std/Test.sol";

import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";
import {GasFaucet} from "../src/GasFaucet.sol";
import {IGasFaucet} from "../src/interfaces/IGasFaucet.sol";
import {GasVoucher} from "../src/GasVoucher.sol";
import {IGasVoucher} from "../src/interfaces/IGasVoucher.sol";
import "../src/utils/Errors.sol";

// Mock contract that rejects ETH transfers
contract RejectETH {
    // No fallback or receive function, so it rejects ETH
}

// Mock contract that accepts ETH
contract AcceptETH {
    receive() external payable {}
}

// Reentrancy attacker contract
contract ReentrancyAttacker {
    GasFaucet public faucet;
    GasVoucher public voucher;
    bool public attackAttempted;
    bool public attackSucceeded;

    constructor(address _faucet, address _voucher) {
        faucet = GasFaucet(payable(_faucet));
        voucher = GasVoucher(_voucher);
    }

    receive() external payable {
        if (!attackAttempted && address(faucet).balance > 0) {
            attackAttempted = true;
            // Try to reenter via redeem (should fail due to reentrancy guard)
            if (voucher.balanceOf(address(this)) > 0) {
                try faucet.redeem(0.1 ether) {
                    attackSucceeded = true; // This should never happen
                } catch {
                    // Attack was blocked by reentrancy guard
                    attackSucceeded = false;
                }
            }
        }
    }
}

contract GasFaucetTest is Test {
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

    // New security tests

    function test_fund_fallback_to_vouchers_when_eth_transfer_fails() public {
        // Deploy a contract that rejects ETH
        RejectETH rejecter = new RejectETH();
        address rejecterAddr = address(rejecter);

        // Fund the faucet with enough ETH
        vm.deal(address(faucet), 10 ether);

        uint256 amount = 1 ether;

        // MPC tries to fund the rejecter contract
        // Should fallback to issuing vouchers instead of reverting
        vm.prank(mpc);
        vm.expectEmit(true, true, true, true);
        emit IGasFaucet.VoucherIssued(rejecterAddr, amount);
        faucet.fund(rejecterAddr, amount);

        // Verify vouchers were issued instead of ETH
        assertEq(rejecterAddr.balance, 0, "No ETH sent");
        assertEq(
            voucher.balanceOf(rejecterAddr),
            amount,
            "Vouchers issued as fallback"
        );
    }

    function test_fund_succeeds_for_contract_that_accepts_eth() public {
        // Deploy a contract that accepts ETH
        AcceptETH accepter = new AcceptETH();
        address accepterAddr = address(accepter);

        // Fund the faucet with enough ETH
        vm.deal(address(faucet), 10 ether);

        uint256 amount = 1 ether;
        uint256 before = accepterAddr.balance;

        // MPC funds the accepter contract
        vm.prank(mpc);
        vm.expectEmit(true, true, true, true);
        emit IGasFaucet.Funded(accepterAddr, amount);
        faucet.fund(accepterAddr, amount);

        // Verify ETH was sent
        assertEq(accepterAddr.balance, before + amount, "ETH sent successfully");
        assertEq(voucher.balanceOf(accepterAddr), 0, "No vouchers issued");
    }

    function test_pause_unpause_only_owner() public {
        vm.deal(address(faucet), 10 ether);

        // Non-owner cannot pause
        vm.prank(alice);
        vm.expectRevert(
            abi.encodeWithSelector(
                Ownable.OwnableUnauthorizedAccount.selector,
                alice
            )
        );
        faucet.pause();

        // Owner can pause
        vm.prank(owner);
        faucet.pause();

        // Fund should fail when paused
        vm.prank(mpc);
        vm.expectRevert();
        faucet.fund(alice, 1 ether);

        // Redeem should fail when paused
        vm.prank(alice);
        vm.expectRevert();
        faucet.redeem(0.5 ether);

        // Non-owner cannot unpause
        vm.prank(alice);
        vm.expectRevert(
            abi.encodeWithSelector(
                Ownable.OwnableUnauthorizedAccount.selector,
                alice
            )
        );
        faucet.unpause();

        // Owner can unpause
        vm.prank(owner);
        faucet.unpause();

        // Should work after unpause
        vm.prank(mpc);
        faucet.fund(alice, 1 ether);
        assertEq(alice.balance, 1 ether);
    }

    function test_reentrancy_protection_on_redeem() public {
        // Setup: Give faucet some ETH and fund the attacker with vouchers
        vm.deal(address(faucet), 10 ether);

        ReentrancyAttacker attacker = new ReentrancyAttacker(
            address(faucet),
            address(voucher)
        );
        address attackerAddr = address(attacker);

        // Fund attacker with vouchers by minting through faucet
        vm.deal(address(faucet), 0.1 ether); // Low balance to trigger voucher issuance
        vm.prank(mpc);
        faucet.fund(attackerAddr, 1 ether);

        assertEq(voucher.balanceOf(attackerAddr), 1 ether, "Attacker has vouchers");

        // Refill faucet
        vm.deal(address(faucet), 10 ether);

        uint256 faucetBalBefore = address(faucet).balance;

        // Attacker tries to redeem and reenter
        vm.prank(attackerAddr);
        faucet.redeem(0.1 ether);

        // Verify the attack was prevented
        // Attacker should only have received 0.1 ETH (not more through reentrancy)
        assertEq(attackerAddr.balance, 0.1 ether, "Only single redeem succeeded");
        assertEq(
            address(faucet).balance,
            faucetBalBefore - 0.1 ether,
            "Faucet only sent 0.1 ETH"
        );
        assertEq(
            voucher.balanceOf(attackerAddr),
            0.9 ether,
            "Correct vouchers burned"
        );
        assertTrue(attacker.attackAttempted(), "Reentry was attempted");
        assertFalse(attacker.attackSucceeded(), "Reentry attack was blocked");
    }

    function test_withdraw_works_even_when_paused() public {
        vm.deal(address(faucet), 10 ether);

        // Pause the faucet
        vm.prank(owner);
        faucet.pause();

        // Withdraw should still work (for emergency recovery)
        uint256 before = bob.balance;
        vm.prank(owner);
        faucet.withdraw(payable(bob), 1 ether);

        assertEq(bob.balance, before + 1 ether, "Withdraw works when paused");
    }

    function test_constructor_reverts_zero_mpc() public {
        vm.expectRevert(ZeroAddress.selector);
        new GasFaucet(address(0), address(voucher), THRESH, owner);
    }

    function test_constructor_reverts_zero_voucher() public {
        vm.expectRevert(ZeroAddress.selector);
        new GasFaucet(mpc, address(0), THRESH, owner);
    }
}
