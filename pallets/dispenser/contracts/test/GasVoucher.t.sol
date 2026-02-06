// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import "forge-std/Test.sol";

import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";
import {IAccessControl} from "@openzeppelin/contracts/access/IAccessControl.sol";
import {GasFaucet} from "../src/GasFaucet.sol";
import {IGasFaucet} from "../src/interfaces/IGasFaucet.sol";
import {GasVoucher} from "../src/GasVoucher.sol";
import {IGasVoucher} from "../src/interfaces/IGasVoucher.sol";
import "../src/utils/Errors.sol";

contract GasVoucherTest is Test {
    GasVoucher voucher;
    address admin = address(0xAD);
    address faucet = address(0xFACADE);
    address rando = address(0xBEEF);

    function setUp() public {
        vm.prank(admin);
        voucher = new GasVoucher(admin);
    }

    function test_setFaucet_only_admin() public {
        bytes32 adminRole = voucher.DEFAULT_ADMIN_ROLE();

        vm.prank(rando);
        vm.expectRevert(
            abi.encodeWithSelector(
                IAccessControl.AccessControlUnauthorizedAccount.selector,
                rando,
                adminRole
            )
        );
        voucher.setFaucet(faucet);

        vm.prank(admin);
        voucher.setFaucet(faucet);
    }

    function test_setFaucet_non_zero() public {
        vm.prank(admin);
        vm.expectRevert(ZeroAddress.selector);
        voucher.setFaucet(address(0));
    }

    function test_faucetMint_only_faucet() public {
        vm.prank(admin);
        voucher.setFaucet(faucet);

        vm.prank(faucet);
        voucher.faucetMint(rando, 100);

        bytes32 faucetRole = voucher.FAUCET_ROLE();

        vm.prank(rando);
        vm.expectRevert(
            abi.encodeWithSelector(
                IAccessControl.AccessControlUnauthorizedAccount.selector,
                rando,
                faucetRole
            )
        );
        voucher.faucetMint(rando, 100);
    }

    function test_faucetBurnFrom_only_faucet() public {
        vm.prank(admin);
        voucher.setFaucet(faucet);

        vm.prank(faucet);
        voucher.faucetMint(rando, 100);
        assertEq(voucher.balanceOf(rando), 100);

        vm.prank(faucet);
        voucher.faucetBurnFrom(rando, 60);
        assertEq(voucher.balanceOf(rando), 40);
    }

    // New security tests

    function test_constructor_reverts_zero_address() public {
        vm.expectRevert(ZeroAddress.selector);
        new GasVoucher(address(0));
    }

    function test_revokeFaucet_only_admin() public {
        vm.prank(admin);
        voucher.setFaucet(faucet);

        bytes32 adminRole = voucher.DEFAULT_ADMIN_ROLE();

        vm.prank(rando);
        vm.expectRevert(
            abi.encodeWithSelector(
                IAccessControl.AccessControlUnauthorizedAccount.selector,
                rando,
                adminRole
            )
        );
        voucher.revokeFaucet(faucet);

        vm.prank(admin);
        vm.expectEmit(true, true, true, true);
        emit IGasVoucher.FaucetRevoked(faucet);
        voucher.revokeFaucet(faucet);

        // Faucet should no longer be able to mint
        bytes32 faucetRole = voucher.FAUCET_ROLE();
        vm.prank(faucet);
        vm.expectRevert(
            abi.encodeWithSelector(
                IAccessControl.AccessControlUnauthorizedAccount.selector,
                faucet,
                faucetRole
            )
        );
        voucher.faucetMint(rando, 100);
    }

    function test_pause_unpause_only_admin() public {
        vm.prank(admin);
        voucher.setFaucet(faucet);

        // Non-admin cannot pause
        bytes32 adminRole = voucher.DEFAULT_ADMIN_ROLE();
        vm.expectRevert(
            abi.encodeWithSelector(
                IAccessControl.AccessControlUnauthorizedAccount.selector,
                rando,
                adminRole
            )
        );
        vm.prank(rando);
        voucher.pause();

        // Admin can pause
        vm.prank(admin);
        voucher.pause();

        // Minting should fail when paused
        vm.prank(faucet);
        vm.expectRevert();
        voucher.faucetMint(rando, 100);

        // Admin can unpause
        vm.prank(admin);
        voucher.unpause();

        // Should work after unpause
        vm.prank(faucet);
        voucher.faucetMint(rando, 100);
        assertEq(voucher.balanceOf(rando), 100);
    }

    function test_transfers_blocked_when_paused() public {
        vm.prank(admin);
        voucher.setFaucet(faucet);

        // Mint some tokens
        vm.prank(faucet);
        voucher.faucetMint(rando, 1000);

        // Transfer should work when not paused
        vm.prank(rando);
        voucher.transfer(admin, 100);
        assertEq(voucher.balanceOf(admin), 100);

        // Pause
        vm.prank(admin);
        voucher.pause();

        // Transfer should fail when paused
        vm.prank(rando);
        vm.expectRevert();
        voucher.transfer(admin, 100);

        // Unpause
        vm.prank(admin);
        voucher.unpause();

        // Transfer should work again
        vm.prank(rando);
        voucher.transfer(admin, 100);
        assertEq(voucher.balanceOf(admin), 200);
    }
}
