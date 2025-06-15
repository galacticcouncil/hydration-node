// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract DelegateCall {
    function delegateCallAddress(address precompile, bytes calldata data) external returns (bool, bytes memory) {
        return precompile.delegatecall(data);
    }

    function callAddress(address precompile, bytes calldata data) external returns (bool, bytes memory) {
        return precompile.call(data);
    }
}