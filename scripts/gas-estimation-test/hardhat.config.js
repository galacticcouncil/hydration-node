require("@nomicfoundation/hardhat-toolbox");

// Charlie's EVM private key (from chain_spec testnet claims)
// EVM address: 0xC19A2970A13ac19898c47d59Cbd0278D428EBC7c
const TEST_PRIVKEY = "653a29ac0c93de0e9f7d7ea2d60338e68f407b18d16d6ff84db996076424f8fa";

/** @type import('hardhat/config').HardhatUserConfig */
module.exports = {
  solidity: "0.8.24",
  networks: {
    local: {
      url: process.env.EVM_RPC_URL || "http://127.0.0.1:9999",
      chainId: 2222222,
      accounts: [TEST_PRIVKEY],
      // High timeout for parachain blocks (~12s)
      timeout: 120000,
      // Prevent gas estimate * multiplier from exceeding block gas limit (60M)
      gasMultiplier: 1,
    }
  }
};
