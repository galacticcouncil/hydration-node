// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

interface IERC20 {
    function transfer(address to, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
}

/// Test receiver for ICE `OnResolved::Forward`: forwards everything it received to the address encoded
/// in `data`, then acks. Used by integration tests.
contract IntentResolutionReceiver {
    event Forwarded(address indexed target, address indexed assetOut, uint256 amount);

    function execute(
        address user,
        uint256 intentId,
        address assetIn,
        uint256 amountIn,
        address assetOut,
        uint256 amountOut,
        bytes calldata data
    ) external returns (bytes4) {
        address target = abi.decode(data, (address));
        uint256 bal = IERC20(assetOut).balanceOf(address(this));
        require(IERC20(assetOut).transfer(target, bal), "transfer failed");
        emit Forwarded(target, assetOut, bal);
        return this.execute.selector;
    }
}
