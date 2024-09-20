require("@nomicfoundation/hardhat-toolbox");
require("@nomicfoundation/hardhat-verify");
const {vars} = require("hardhat/config");
const PRIVKEY = vars.get("PRIVKEY", "42d8d953e4f9246093a33e9ca6daa078501012f784adfe4bbed57918ff13be14");

/** @type import('hardhat/config').HardhatUserConfig */
module.exports = {
  solidity: "0.8.24",
  networks: {
    nice: {
      url: "https://rpc.nice.hydration.cloud",
      accounts: [PRIVKEY]
    }
  },
  etherscan: {
    apiKey: {
      nice: "nice"
    },
    customChains: [
      {
        network: "nice",
        chainId: 222222,
        urls: {
          apiURL: "https://blockscout.nice.hydration.cloud/api",
          browserURL: "https://blockscout.nice.hydration.cloud"
        }
      }
    ]
  }
};
