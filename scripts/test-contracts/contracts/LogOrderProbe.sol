// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.24;

interface IERC20 {
    function transfer(address to, uint256 amount) external returns (bool);
}

/// Probe contract used to verify EVM log ordering.
///
/// `exercise(token, to, amount)` emits Marker(0), then calls
/// token.transfer(to, amount) (which goes through the multicurrency
/// precompile and triggers the substrate-side hook), then emits Marker(1).
///
/// In the resulting receipt logs we expect log_index ordering:
///   [Marker(0), Transfer(from=this, to=to, value=amount), Marker(1)]
///
/// — proving that substrate-hook-emitted logs land at the precompile's
/// call site (via the per-precompile drain) and are NOT bunched at the
/// end of the EVM frame's logs.
contract LogOrderProbe {
    event Marker(uint256 idx);

    function exercise(address token, address to, uint256 amount) external {
        emit Marker(0);
        IERC20(token).transfer(to, amount);
        emit Marker(1);
    }
}
