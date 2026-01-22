// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {GasFaucet} from "../src/GasFaucet.sol";
import {GasVoucher} from "../src/GasVoucher.sol";

contract GasFaucetScript is Script {
    uint256 public constant MIN_ETH_THRESHOLD = 0.1 ether;
    uint256 public constant INITIAL_FUNDING = 1 ether;

    function run() public {
        // Load deployer private key & MPC from environment
        // export PRIVATE_KEY=0x...
        // export MPC_ADDRESS=0x...
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(deployerKey);
        address mpc = vm.envAddress("MPC_ADDRESS");

        vm.startBroadcast(deployerKey);

        console.log("Deployer:", deployer);
        console.log("MPC address:", mpc);

        // Deploy GasVoucher
        GasVoucher gasVoucher = new GasVoucher(deployer);
        console.log("Deployed GasVoucher at:", address(gasVoucher));

        // Deploy GasFaucet
        GasFaucet gasFaucet = new GasFaucet(
            mpc,
            address(gasVoucher),
            MIN_ETH_THRESHOLD,
            deployer
        );
        console.log("Deployed GasFaucet at:", address(gasFaucet));

        // Wire voucher -> faucet
        gasVoucher.setFaucet(address(gasFaucet));
        console.log("Set GasFaucet as faucet in GasVoucher");

        // Fund the faucet with a small amount of ETH
        (bool success, ) = payable(address(gasFaucet)).call{
            value: INITIAL_FUNDING
        }("");
        require(success, "Funding GasFaucet failed");

        console.log("Funded GasFaucet with:", INITIAL_FUNDING);
        console.log("GasFaucet ETH balance:", address(gasFaucet).balance);

        vm.stopBroadcast();
    }
}
