// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.10;

import {IERC20} from '../../dependencies/openzeppelin/contracts/IERC20.sol';
import {GPv2SafeERC20} from '../../dependencies/gnosis/contracts/GPv2SafeERC20.sol';
import {AToken} from './AToken.sol';
import {IPool} from '../../interfaces/IPool.sol';

interface ILockManager {
    function getLockedBalance(address token, address account) external view returns (uint256);
}

contract LockableAToken is AToken {
    using GPv2SafeERC20 for IERC20;

    address public constant LOCK_MANAGER = 0x0000000000000000000000000000000000000806;

    error ExceedsFreeBalance(uint256 requested, uint256 available);

    constructor(IPool pool) AToken(pool) {}

    function getFreeBalance(address account) public view returns (uint256) {
        uint256 total = balanceOf(account);
        uint256 locked = getLockedBalance(account);
        return locked >= total ? 0 : total - locked;
    }

    function getLockedBalance(address account) public view returns (uint256) {
        return ILockManager(LOCK_MANAGER).getLockedBalance(address(this), account);
    }

    function burn(
        address from,
        address receiverOfUnderlying,
        uint256 amount,
        uint256 index
    ) external virtual override onlyPool {
        uint256 freeBalance = getFreeBalance(from);
        if (amount > freeBalance) revert ExceedsFreeBalance(amount, freeBalance);

        _burnScaled(from, receiverOfUnderlying, amount, index);
        if (receiverOfUnderlying != address(this)) {
            IERC20(_underlyingAsset).safeTransfer(receiverOfUnderlying, amount);
        }
    }

    function _transfer(
        address from,
        address to,
        uint256 amount,
        bool validate
    ) internal virtual override {
        uint256 freeBalance = getFreeBalance(from);
        if (amount > freeBalance) revert ExceedsFreeBalance(amount, freeBalance);
        super._transfer(from, to, amount, validate);
    }
}
