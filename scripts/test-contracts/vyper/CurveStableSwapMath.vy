# @version ^0.4.0

# Faithful extraction of Curve StableSwap math from StableSwap3Pool.vy
# Source: https://github.com/curvefi/curve-contract/blob/master/contracts/pools/3pool/StableSwap3Pool.vy
#
# Changes from original:
#   - Internal functions made external with @pure decorator
#   - Fixed-size arrays (uint256[N_COINS]) replaced with DynArray[uint256, 8]
#   - N_COINS derived from array length at runtime
#   - No state, no tokens, no admin — pure math only
#
# Compiled with: vyper 0.4.3
# Command: vyper scripts/test-contracts/vyper/CurveStableSwapMath.vy -f bytecode

FEE_DENOMINATOR: constant(uint256) = 10 ** 10


@internal
@pure
def _get_D(xp: DynArray[uint256, 8], amp: uint256) -> uint256:
    """
    Curve's get_D — Newton's method for the StableSwap invariant D.
    Solves: A*n^n*sum(x_i) + D = A*D*n^n + D^(n+1)/(n^n * prod(x_i))
    """
    N_COINS: uint256 = len(xp)
    S: uint256 = 0
    for x: uint256 in xp:
        S += x
    if S == 0:
        return 0

    Dprev: uint256 = 0
    D: uint256 = S
    Ann: uint256 = amp * N_COINS
    for _i: uint256 in range(255):
        D_P: uint256 = D
        for x: uint256 in xp:
            D_P = D_P * D // (x * N_COINS)
        Dprev = D
        D = (Ann * S + D_P * N_COINS) * D // ((Ann - 1) * D + (N_COINS + 1) * D_P)
        # Convergence check
        if D > Dprev:
            if D - Dprev <= 1:
                break
        else:
            if Dprev - D <= 1:
                break
    return D


@external
@pure
def get_D(xp: DynArray[uint256, 8], amp: uint256) -> uint256:
    return self._get_D(xp, amp)


@internal
@pure
def _get_y(i: uint256, j: uint256, x: uint256, xp: DynArray[uint256, 8], amp: uint256) -> uint256:
    """
    Curve's get_y — solve for the new balance of coin j.
    Given coin i has new balance x, find y_j that satisfies the invariant.
    """
    N_COINS: uint256 = len(xp)
    assert i != j
    assert j < N_COINS
    assert i < N_COINS

    D: uint256 = self._get_D(xp, amp)
    c: uint256 = D
    S_: uint256 = 0
    Ann: uint256 = amp * N_COINS

    _x: uint256 = 0
    for _i: uint256 in range(8):
        if _i >= N_COINS:
            break
        if _i == i:
            _x = x
        elif _i != j:
            _x = xp[_i]
        else:
            continue
        S_ += _x
        c = c * D // (_x * N_COINS)
    c = c * D // (Ann * N_COINS)
    b: uint256 = S_ + D // Ann
    y_prev: uint256 = 0
    y: uint256 = D
    for _i: uint256 in range(255):
        y_prev = y
        y = (y * y + c) // (2 * y + b - D)
        # Convergence check
        if y > y_prev:
            if y - y_prev <= 1:
                break
        else:
            if y_prev - y <= 1:
                break
    return y


@external
@pure
def get_y(i: uint256, j: uint256, x: uint256, xp: DynArray[uint256, 8], amp: uint256) -> uint256:
    return self._get_y(i, j, x, xp, amp)


@internal
@pure
def _get_y_D(amp: uint256, i: uint256, xp: DynArray[uint256, 8], D: uint256) -> uint256:
    """
    Curve's get_y_D — solve for balance of coin i at a given D value.
    Used by calc_withdraw_one_coin.
    """
    N_COINS: uint256 = len(xp)
    assert i < N_COINS

    c: uint256 = D
    S_: uint256 = 0
    Ann: uint256 = amp * N_COINS

    _x: uint256 = 0
    for _i: uint256 in range(8):
        if _i >= N_COINS:
            break
        if _i != i:
            _x = xp[_i]
        else:
            continue
        S_ += _x
        c = c * D // (_x * N_COINS)
    c = c * D // (Ann * N_COINS)
    b: uint256 = S_ + D // Ann
    y_prev: uint256 = 0
    y: uint256 = D
    for _i: uint256 in range(255):
        y_prev = y
        y = (y * y + c) // (2 * y + b - D)
        if y > y_prev:
            if y - y_prev <= 1:
                break
        else:
            if y_prev - y <= 1:
                break
    return y


@external
@pure
def get_y_D(amp: uint256, i: uint256, xp: DynArray[uint256, 8], D: uint256) -> uint256:
    return self._get_y_D(amp, i, xp, D)


@external
@pure
def calc_token_amount(
    old_balances: DynArray[uint256, 8],
    new_balances: DynArray[uint256, 8],
    amp: uint256,
    token_supply: uint256,
    fee: uint256,
) -> uint256:
    """
    Curve's add_liquidity share calculation.
    Returns the number of LP tokens minted for a deposit.
    fee is in units of FEE_DENOMINATOR (10^10). Pass 0 to disable fees.
    """
    N_COINS: uint256 = len(old_balances)
    assert len(new_balances) == N_COINS

    D0: uint256 = self._get_D(old_balances, amp)
    D1: uint256 = self._get_D(new_balances, amp)
    assert D1 > D0

    D2: uint256 = D1
    if token_supply > 0 and fee > 0:
        _fee: uint256 = fee * N_COINS // (4 * (N_COINS - 1))
        adjusted: DynArray[uint256, 8] = []
        for i: uint256 in range(8):
            if i >= N_COINS:
                break
            ideal_balance: uint256 = D1 * old_balances[i] // D0
            difference: uint256 = 0
            if ideal_balance > new_balances[i]:
                difference = ideal_balance - new_balances[i]
            else:
                difference = new_balances[i] - ideal_balance
            adjusted.append(new_balances[i] - (_fee * difference // FEE_DENOMINATOR))
        D2 = self._get_D(adjusted, amp)

    if token_supply == 0:
        return D1
    else:
        return token_supply * (D2 - D0) // D0


@external
@pure
def calc_withdraw_one_coin(
    balances: DynArray[uint256, 8],
    token_amount: uint256,
    i: uint256,
    total_supply: uint256,
    amp: uint256,
    fee: uint256,
) -> (uint256, uint256):
    """
    Curve's _calc_withdraw_one_coin math.
    Returns (dy, dy_fee) — amount received and fee amount.
    fee is in units of FEE_DENOMINATOR (10^10). Pass 0 to disable fees.
    """
    N_COINS: uint256 = len(balances)
    assert i < N_COINS

    _fee: uint256 = 0
    if fee > 0:
        _fee = fee * N_COINS // (4 * (N_COINS - 1))

    D0: uint256 = self._get_D(balances, amp)
    D1: uint256 = D0 - token_amount * D0 // total_supply

    new_y: uint256 = self._get_y_D(amp, i, balances, D1)
    dy_0: uint256 = balances[i] - new_y

    xp_reduced: DynArray[uint256, 8] = []
    for j: uint256 in range(8):
        if j >= N_COINS:
            break
        dx_expected: uint256 = 0
        if j == i:
            dx_expected = balances[j] * D1 // D0 - new_y
        else:
            dx_expected = balances[j] - balances[j] * D1 // D0
        xp_reduced.append(balances[j] - (_fee * dx_expected // FEE_DENOMINATOR))

    dy: uint256 = xp_reduced[i] - self._get_y_D(amp, i, xp_reduced, D1)
    dy = dy - 1  # Withdraw less to account for rounding errors

    dy_fee: uint256 = dy_0 - dy

    return (dy, dy_fee)


@external
@pure
def get_dy(i: uint256, j: uint256, dx: uint256, balances: DynArray[uint256, 8], amp: uint256, fee: uint256) -> uint256:
    """
    Curve's get_dy — calculate swap output amount.
    Returns the amount of coin j received for dx of coin i, after fee.
    fee is in units of FEE_DENOMINATOR (10^10). Pass 0 to disable fees.
    """
    x: uint256 = balances[i] + dx
    y: uint256 = self._get_y(i, j, x, balances, amp)
    dy: uint256 = balances[j] - y - 1
    if fee > 0:
        _fee: uint256 = fee * dy // FEE_DENOMINATOR
        return dy - _fee
    return dy
