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
}
