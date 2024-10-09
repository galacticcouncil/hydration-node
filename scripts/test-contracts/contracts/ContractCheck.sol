// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.24;

import "./Address.sol";

contract ContractCheck {

    event Checked(address indexed addr);

    function isContract(address _addr) public view returns (bool) {
        return Address.isContract(_addr);
    }

    function check(address _addr) public {
        require(isContract(_addr), 'addr is not a contract');
        emit Checked(_addr);
    }
}
