// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.24;

// probes whether a precompile can be reached via DELEGATECALL / CALLCODE
contract DelegateProbe {
    function probeDelegate(address target, bytes calldata data) external returns (bool ok, bytes memory ret) {
        (ok, ret) = target.delegatecall(data);
    }

    function probeCall(address target, bytes calldata data) external returns (bool ok, bytes memory ret) {
        (ok, ret) = target.call(data);
    }
}
