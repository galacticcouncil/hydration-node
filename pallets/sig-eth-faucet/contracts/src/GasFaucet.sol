// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

contract GasFaucet {
    address public owner;
    address public mpc;

    event Funded(address indexed to, uint256 amount);

    modifier onlyOwner() {
        require(msg.sender == owner, "owner");
        _;
    }

    modifier onlyMPC() {
        require(msg.sender == mpc, "mpc");
        _;
    }

    constructor(address _mpc) {
        owner = msg.sender;
        mpc = _mpc;
    }

    function setMPC(address _mpc) external onlyOwner {
        require(_mpc != address(0), "zero");
        mpc = _mpc;
    }

    function fund(address to, uint256 amount) external onlyMPC {
        require(to != address(0), "zero");
        (bool ok, ) = payable(to).call{value: amount}("");
        require(ok, "send");
        emit Funded(to, amount);
    }

    function withdraw(address payable to, uint256 amount) external onlyOwner {
        (bool ok, ) = to.call{value: amount}("");
        require(ok, "withdraw");
    }

    receive() external payable {}
}
