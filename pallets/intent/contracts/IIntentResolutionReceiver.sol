// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.0;

/// @title ICE intent-resolution receiver
/// @notice Implemented by a contract named in an intent's `OnResolved::Forward` action. On (per-trade)
/// resolution the runtime pushes `amountOut` of `assetOut` to this contract and then calls `execute`.
/// The call runs inside one storage transaction: returning anything other than `this.execute.selector`
/// — or reverting — rolls the push back and leaves the funds with the intent owner.
interface IIntentResolutionReceiver {
    /// @param user      Intent owner's EVM address. `msg.sender == user` (identity only).
    /// @param intentId  The resolved intent's id.
    /// @param assetIn   ERC20-mapped address of the intent's input asset.
    /// @param amountIn  Input amount for this trade.
    /// @param assetOut  ERC20-mapped address of the output asset (pushed to this contract).
    /// @param amountOut Output amount pushed to this contract before the call (this trade's fill).
    /// @param data      Opaque, contract-defined payload supplied at intent submission.
    /// @return The `bytes4` ack: must equal `this.execute.selector`.
    function execute(
        address user,
        uint256 intentId,
        address assetIn,
        uint256 amountIn,
        address assetOut,
        uint256 amountOut,
        bytes calldata data
    ) external returns (bytes4);
}
