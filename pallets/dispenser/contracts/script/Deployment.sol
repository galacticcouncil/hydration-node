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

        console.log("Broadcasting from:", msg.sender);
        console.log("MPC address:", mpc);

        gasVoucher = new GasVoucher();
        console.log("Deployed GasVoucher at:", address(gasVoucher));

        gasFaucet = new GasFaucet(mpc, address(gasVoucher), 1 ether);
        console.log("Deployed GasFaucet at:", address(gasFaucet));

        gasVoucher.setFaucet(address(gasFaucet));
        console.log("Set GasFaucet as faucet in GasVoucher");

        (bool success, ) = payable(address(gasFaucet)).call{
            value: 0.00001 ether
        }("");
        require(success, "Funding GasFaucet failed");

        console.log("Funded GasFaucet with 100 ETH");
        console.log("GasFaucet ETH balance:", address(gasFaucet).balance);

        vm.stopBroadcast();
    }
}
