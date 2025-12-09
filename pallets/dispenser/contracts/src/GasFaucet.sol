// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";
import {IGasVoucher} from "./interfaces/IGasVoucher.sol";
import {IGasFaucet} from "./interfaces/IGasFaucet.sol";
import "./utils/Errors.sol";

/// @title GasFaucet
/// @notice Sends ETH to users when there is sufficient balance; otherwise issues
///         vouchers that can be redeemed later for ETH.
/// @dev
/// - `mpc` is an off-chain-controlled address allowed to call `fund`.
/// - `minEthThreshold` is a guardrail so the faucet doesn't fully drain via `fund`.
/// - When the faucet cannot send ETH without dropping below the threshold,
///   it mints `GasVoucher` IOUs instead.
contract GasFaucet is Ownable, IGasFaucet {
    // ========= State =========

    /// @inheritdoc IGasFaucet
    address public override mpc;

    /// @inheritdoc IGasFaucet
    uint256 public override minEthThreshold;

    /// @inheritdoc IGasFaucet
    IGasVoucher public override voucher;

    // ========= Modifiers =========

    /// @notice Restricts a function to be callable only by the MPC.
    modifier onlyMPC() {
        if (msg.sender != mpc) {
            revert NotMPC();
        }
        _;
    }

    // ========= Constructor =========

    /// @notice Deploys the GasFaucet contract.
    /// @param _mpc Initial MPC address allowed to call `fund`.
    /// @param _voucher Address of the `GasVoucher` contract.
    /// @param _minEthThreshold Initial minimum ETH threshold (in wei).
    constructor(
        address _mpc,
        address _voucher,
        uint256 _minEthThreshold,
        address _owner
    ) Ownable(_owner) {
        if (_mpc == address(0)) {
            revert ZeroAddress();
        }
        if (_voucher == address(0)) {
            revert ZeroAddress();
        }

        mpc = _mpc;
        minEthThreshold = _minEthThreshold;
        voucher = IGasVoucher(_voucher);
    }

    // ========= Admin Functions (owner) =========

    /// @inheritdoc IGasFaucet
    function setMPC(address _mpc) external override onlyOwner {
        if (_mpc == address(0)) {
            revert ZeroAddress();
        }

        mpc = _mpc;
        emit MPCUpdated(_mpc);
    }

    /// @inheritdoc IGasFaucet
    function setMinEthThreshold(
        uint256 _thresholdWei
    ) external override onlyOwner {
        minEthThreshold = _thresholdWei;
        emit ThresholdUpdated(_thresholdWei);
    }

    /// @inheritdoc IGasFaucet
    function setVoucher(address _voucher) external override onlyOwner {
        if (_voucher == address(0)) {
            revert ZeroAddress();
        }

        voucher = IGasVoucher(_voucher);
        emit VoucherUpdated(_voucher);
    }

    // ========= Core Logic =========

    /// @inheritdoc IGasFaucet
    function fund(address to, uint256 amountWei) external override onlyMPC {
        if (to == address(0)) {
            revert ZeroAddress();
        }
        if (amountWei == 0) {
            revert ZeroAmount();
        }

        uint256 balance = address(this).balance;

        // Only if we have enough and will remain above threshold.
        if (balance >= amountWei && balance - amountWei > minEthThreshold) {
            (bool ok, ) = payable(to).call{value: amountWei}("");
            if (!ok) {
                revert EthTransferFailed();
            }
            emit Funded(to, amountWei);
        } else {
            // Otherwise, issue vouchers as IOUs.
            voucher.faucetMint(to, amountWei);
            emit VoucherIssued(to, amountWei);
        }
    }

    /// @inheritdoc IGasFaucet
    function redeem(uint256 amountWei) external override {
        if (amountWei == 0) {
            revert ZeroAmount();
        }

        if (address(this).balance < amountWei) {
            revert FaucetLowBalance();
        }

        // Burn vouchers and then send ETH.
        voucher.faucetBurnFrom(msg.sender, amountWei);

        (bool ok, ) = payable(msg.sender).call{value: amountWei}("");
        if (!ok) {
            revert EthTransferFailed();
        }

        emit Redeemed(msg.sender, amountWei);
    }

    /// @inheritdoc IGasFaucet
    function withdraw(
        address payable to,
        uint256 amountWei
    ) external override onlyOwner {
        if (to == address(0)) {
            revert ZeroAddress();
        }
        if (amountWei == 0) {
            revert ZeroAmount();
        }
        if (address(this).balance < amountWei) {
            revert FaucetLowBalance();
        }

        (bool ok, ) = to.call{value: amountWei}("");
        if (!ok) {
            revert EthTransferFailed();
        }

        emit Withdrawn(to, amountWei);
    }

    /// @notice Accepts raw ETH deposits into the faucet.
    receive() external payable {}
}
