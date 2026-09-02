// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.24;

contract Minimal {
    uint256 public value;

    constructor(uint256 _value) {
        value = _value;
    }
}

// deploys via inner CREATE / CREATE2, which are guarded by CreateInnerOriginFilter,
// not by the deployer whitelist that guards top-level creates
contract ContractFactory {
    event Deployed(address addr);

    function create(uint256 value) public returns (address) {
        Minimal m = new Minimal(value);
        emit Deployed(address(m));
        return address(m);
    }

    function create2(uint256 value, bytes32 salt) public returns (address) {
        Minimal m = new Minimal{salt: salt}(value);
        emit Deployed(address(m));
        return address(m);
    }
}
