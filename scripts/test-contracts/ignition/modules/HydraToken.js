const { buildModule } = require("@nomicfoundation/hardhat-ignition/modules");

const HydraToken = buildModule("HydraToken", (m) => {
  const token = m.contract("HydraToken");

  return { token };
});

module.exports = HydraToken;
