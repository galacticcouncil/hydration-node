// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.24;

interface IDispatcher {
    function dispatch(bytes calldata) external returns (bool);
}

/// Probe for the "revert with substrate state changes pending" path.
///
/// `try_dispatch_then_revert(target, data)` calls `target.call(data)` (assumed
/// to be the dispatcher precompile) and then unconditionally reverts.
/// The dispatch's substrate state changes get rolled back by pallet_evm's
/// `with_storage_layer` wrapping; the dispatcher's inline-emitted logs get
/// dropped by frontier when the outer frame reverts (substate logs are
/// cleared on revert). This contract verifies that NO Transfer logs leak
/// into the resulting eth-tx receipt or the synthetic-logs buffer when the
/// outer EVM frame reverts.
contract RevertingDispatcher {
    event Marker(uint256 idx);

    function try_dispatch_then_revert(address dispatcher, bytes calldata sub_call_data) external {
        emit Marker(0);
        (bool ok, ) = dispatcher.call(sub_call_data);
        require(ok, "dispatch failed");
        emit Marker(1);
        revert("intentional revert after dispatch");
    }
}
