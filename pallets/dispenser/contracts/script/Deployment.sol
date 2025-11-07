// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {GasFaucet} from "../src/GasFaucet.sol";
import {GasVoucher} from "../src/GasVoucher.sol";

contract GasFaucetScript is Script {
    GasFaucet public gasFaucet;
    GasVoucher public gasVoucher;
    address public mpc = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266;

    function setUp() public {}

    function run() public {
        vm.startBroadcast();

        gasVoucher = new GasVoucher();

        gasFaucet = new GasFaucet(mpc, address(gasVoucher), 1 ether);

        gasVoucher.setFaucet(address(gasFaucet));

        vm.stopBroadcast();
    }
}
