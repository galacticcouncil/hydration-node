// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {GasVoucher} from "./GasVoucher.sol";

contract GasFaucet {
    address public owner;
    address public mpc;

    uint256 public minEthThreshold;

    GasVoucher public immutable voucher;

    event Funded(address indexed to, uint256 amountWei);
    event VoucherIssued(address indexed to, uint256 amountWei);
    event Redeemed(address indexed redeemer, uint256 amountWei);
    event Withdrawn(address indexed to, uint256 amountWei);
    event MPCUpdated(address indexed newMpc);
    event ThresholdUpdated(uint256 newThreshold);

    modifier onlyOwner() {
        require(msg.sender == owner, "owner");
        _;
    }

    modifier onlyMPC() {
        require(msg.sender == mpc, "mpc");
        _;
    }

    constructor(address _mpc, address _voucher, uint256 _minEthThreshold) {
        owner = msg.sender;
        require(_mpc != address(0), "zero mpc");
        require(_voucher != address(0), "zero voucher");
        mpc = _mpc;
        minEthThreshold = _minEthThreshold;
        voucher = GasVoucher(_voucher);
    }

    function setMPC(address _mpc) external onlyOwner {
        require(_mpc != address(0), "zero");
        mpc = _mpc;
        emit MPCUpdated(_mpc);
    }

    function setMinEthThreshold(uint256 _thresholdWei) external onlyOwner {
        minEthThreshold = _thresholdWei;
        emit ThresholdUpdated(_thresholdWei);
    }

    function fund(address to, uint256 amountWei) external onlyMPC {
        require(to != address(0), "zero");
        require(amountWei > 0);
        uint256 balance = address(this).balance;
        if (balance >= amountWei && balance - amountWei >= minEthThreshold) {
            (bool ok, ) = payable(to).call{value: amountWei}("");
            require(ok, "send");
            emit Funded(to, amountWei);
        } else {
            voucher.faucetMint(to, amountWei);
            emit VoucherIssued(to, amountWei);
        }
    }

    function redeem(uint256 amountWei) external {
        require(amountWei > 0, "zero amt");
        require(address(this).balance >= amountWei, "faucet low");
        voucher.faucetBurnFrom(msg.sender, amountWei);
        (bool ok, ) = payable(msg.sender).call{value: amountWei}("");
        require(ok, "redeem send");
        emit Redeemed(msg.sender, amountWei);
    }

    function withdraw(
        address payable to,
        uint256 amountWei
    ) external onlyOwner {
        require(to != address(0), "zero to");
        (bool ok, ) = to.call{value: amountWei}("");
        require(ok, "withdraw");
        emit Withdrawn(to, amountWei);
    }

    receive() external payable {}
}
