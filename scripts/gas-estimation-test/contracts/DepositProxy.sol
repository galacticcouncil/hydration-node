// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/// @title MockStablecoin - Simple ERC20 token for gas estimation testing
contract MockStablecoin is ERC20 {
    constructor() ERC20("Mock Stablecoin", "MUSD") {
        _mint(msg.sender, 1_000_000 * 10 ** 18);
    }
}

/// @title BalanceTracker - External contract tracking deposit balances
contract BalanceTracker {
    mapping(address => uint256) public balances;

    function record(address user, uint256 amount) external {
        balances[user] += amount;
    }
}

/// @title RewardsAccumulator - External contract tracking rewards
contract RewardsAccumulator {
    mapping(address => uint256) public pending;

    function accrue(address user, uint256 amount) external {
        pending[user] += amount;
    }
}

/// @title FeeVault - External contract collecting protocol fees
contract FeeVault {
    uint256 public totalFees;
    mapping(address => uint256) public feesPaid;

    function collect(address payer, uint256 fee) external {
        feesPaid[payer] += fee;
        totalFees += fee;
    }
}

/// @title OracleNotifier - External contract for oracle notifications
contract OracleNotifier {
    mapping(address => uint256) public lastAction;

    function notify(address user) external {
        lastAction[user] = block.timestamp;
    }
}

/// @title RegistryUpdater - External contract for participation tracking
contract RegistryUpdater {
    mapping(address => uint256) public actionCount;

    function increment(address user) external {
        actionCount[user] += 1;
    }
}

/// @title DepositProxy - Mimics DecentralPool's deposit pattern
/// @notice Makes 6 external calls per deposit, triggering EIP-150's 63/64th gas rule.
///
/// The Problem:
///   Without binary search gas estimation, the EVM executor runs deposit() with
///   the full block gas limit (~15M or more). Each external call forwards 63/64
///   of the remaining gas to the subcall. The reported "gas used" reflects all
///   the gas that was *forwarded*, not just what was actually consumed.
///   This inflates eth_estimateGas to near the block gas limit (~15-45M)
///   instead of the actual ~200K needed.
///
/// The Fix:
///   Enabling `rpc-binary-search-estimate` in fc-rpc performs a binary search
///   between 21K and block_gas_limit to find the minimum gas that allows the
///   transaction to succeed, returning a tight ~200K estimate.
contract DepositProxy {
    IERC20 public token;
    BalanceTracker public balanceTracker;
    RewardsAccumulator public rewards;
    FeeVault public feeVault;
    OracleNotifier public oracle;
    RegistryUpdater public registry;

    event Deposited(address indexed user, uint256 amount);

    constructor(address _token) {
        token = IERC20(_token);
        balanceTracker = new BalanceTracker();
        rewards = new RewardsAccumulator();
        feeVault = new FeeVault();
        oracle = new OracleNotifier();
        registry = new RegistryUpdater();
    }

    /// @notice Deposit tokens - makes 6 external calls
    function deposit(uint256 amount) external {
        // External call 1: Transfer tokens from user
        require(token.transferFrom(msg.sender, address(this), amount), "transfer failed");

        // External call 2: Record deposit balance
        balanceTracker.record(msg.sender, amount);

        // External call 3: Accrue rewards (1% of deposit)
        rewards.accrue(msg.sender, amount / 100);

        // External call 4: Collect protocol fee (0.1%)
        feeVault.collect(msg.sender, amount / 1000);

        // External call 5: Notify oracle
        oracle.notify(msg.sender);

        // External call 6: Update registry
        registry.increment(msg.sender);

        emit Deposited(msg.sender, amount);
    }
}
