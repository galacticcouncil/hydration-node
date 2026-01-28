// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {GasFaucet} from "../src/GasFaucet.sol";
import {GasVoucher} from "../src/GasVoucher.sol";

contract GasFaucetScript is Script {
    uint256 public constant MIN_ETH_THRESHOLD = 0.1 ether;
    uint256 public constant INITIAL_FUNDING = 1 ether;
    // --------DO NOT CHANGE--------
    // Keep these constant forever if we want the *same* CREATE2 addresses every time.
    // If you change salts OR constructor args, the address changes.
    bytes32 internal constant SALT_VOUCHER = keccak256("GAS_VOUCHER_V1");
    bytes32 internal constant SALT_FAUCET  = keccak256("GAS_FAUCET_V1");

    function run() public {
        // export PRIVATE_KEY=0x...
        // export MPC_ADDRESS=0x...
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(deployerKey);
        address mpc = vm.envAddress("MPC_ADDRESS");

        vm.startBroadcast(deployerKey);

        console.log("Deployer:", deployer);
        console.log("MPC address:", mpc);

        // -------------------------
        // CREATE2 deployments
        // -------------------------

        // Deploy GasVoucher via CREATE2
        GasVoucher gasVoucher = new GasVoucher{salt: SALT_VOUCHER}(deployer);
        console.log("Deployed GasVoucher (CREATE2) at:", address(gasVoucher));

        // Deploy GasFaucet via CREATE2
        GasFaucet gasFaucet = new GasFaucet{salt: SALT_FAUCET}(
            mpc,
            address(gasVoucher),
            MIN_ETH_THRESHOLD,
            deployer
        );
        console.log("Deployed GasFaucet (CREATE2) at:", address(gasFaucet));

        // Wire voucher -> faucet
        gasVoucher.setFaucet(address(gasFaucet));
        console.log("Set GasFaucet as faucet in GasVoucher");

        // Fund the faucet
        (bool success, ) = payable(address(gasFaucet)).call{value: INITIAL_FUNDING}("");
        require(success, "Funding GasFaucet failed");

        console.log("Funded GasFaucet with:", INITIAL_FUNDING);
        console.log("GasFaucet ETH balance:", address(gasFaucet).balance);

        vm.stopBroadcast();
    }
}
