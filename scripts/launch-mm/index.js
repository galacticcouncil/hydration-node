const {ApiPromise, WsProvider} = require('@polkadot/api');
const ethers = require('ethers');

const log = console.log;
console.log = () => null;
console.warn = () => null;

function account(address) {
  const prefix = Buffer.from('ETH\0');
  const addressBuffer = Buffer.from(address.replace('0x', ''), 'hex');
  const remainingBytes = 32 - prefix.length - addressBuffer.length;
  const padding = Buffer.alloc(remainingBytes);
  return '0x' + Buffer.concat([prefix, addressBuffer, padding]).toString('hex');
}

function padAddress(address) {
  const stripped = address.replace('0x', '');
  const padded = stripped.padEnd(64, '0');
  return '0x' + padded;
}

const supplyData = ({assetAddress, amount, onBehalfOf, referralCode = 0}) =>
  new ethers.utils.Interface(["function supply(address asset, uint256 amount, address onBehalfOf, uint16 referralCode)"])
    .encodeFunctionData("supply", [assetAddress, amount, onBehalfOf, referralCode]);
const pausePoolData = (paused = true) => new ethers.utils.Interface(["function setPoolPause(bool)"]).encodeFunctionData("setPoolPause", [paused]);
const reservePauseData = (asset, paused = true) => new ethers.utils.Interface(["function setReservePause(address,bool)"]).encodeFunctionData("setReservePause", [asset, paused]);
const addEmergencyAdminData = (admin) => new ethers.utils.Interface(["function addEmergencyAdmin(address)"]).encodeFunctionData("addEmergencyAdmin", [admin]);

const location = contract => ({
  "parents": "0",
  "interior": {
    "X1": {
      "AccountKey20": {
        "network": null,
        "key": contract
      }
    }
  }
});

// Configuration
const config = {
  EMERGENCY_ADMIN: "7J4KqjeRmGZPVEAogDgtxVenmsJcsvPBCySdDGxaKQ6Yyknj",
  AAVE_POOL: "0x1b02E051683b5cfaC5929C25E84adb26ECf87B38",
  POOL_CONFIGURATOR: "0xE64C38E2Fa00DFe4F1d0B92f75B8E44eBDF292e4",
  ACL_MANAGER: "0x8c5E657CA8879ada34555130F3Be255ae47558B5",
  ADMIN: "0xAa7e0000000000000000000000000000000Aa7e0", // 7KATdGbEJQzoAfojGx5ScN3kEExXwhCX1yENi4PtSwgdkn4i
  WBTC: "0x0000000000000000000000000000000100000013",
  DOT: "0x0000000000000000000000000000000100000005",
  USDC: "0x0000000000000000000000000000000100000016",
  USDT: "0x000000000000000000000000000000010000000a"
};

async function generateProposal() {
  const provider = new WsProvider(process.env.RPC || 'wss://rpc.hydradx.cloud');
  const api = await ApiPromise.create({provider});
  const {utility, evm, evmAccounts, assetRegistry} = api.tx;

  const evmAddress = account => ethers.utils.hexlify(api.createType('AccountId', account).toU8a().slice(0, 20));

  const evmCall = ({from, to, data}) =>
    evm.call(evmAddress(from), to, data, "0", "1000000", "600000000", undefined, undefined, []);
  const rootEvmCall = ({from, to, data}) => utility.dispatchAs(
    {system: {signed: from}},
    evmCall({from, to, data})
  );
  const supplyCall = (from, assetAddress, amount) => rootEvmCall({
    from,
    to: config.AAVE_POOL,
    data: supplyData({assetAddress, amount, onBehalfOf: evmAddress(from)})
  });

  const treasuryAccount = "7L53bUTBopuwFt3mKUfmkzgGLayYa1Yvn1hAg9v5UMrQzTfh";

  const batch = [
    evmAccounts.approveContract(config.AAVE_POOL),
    utility.dispatchAs({system: {signed: treasuryAccount}}, evmAccounts.bindEvmAddress()),
    rootEvmCall({
      from: padAddress(config.ADMIN),
      to: config.ACL_MANAGER,
      data: addEmergencyAdminData(evmAddress(config.EMERGENCY_ADMIN))
    }),
    assetRegistry.register(1001, "aDOT", "Erc20", 0, "aDOT", 10, location("0x02639ec01313c8775Fae74F2dad1118c8A8a86dA"), null, true),
    assetRegistry.register(1002, "aUSDT", "Erc20", 0, "aUSDT", 6, location("0xc64980E4eAF9A1151bd21712b9946B81e41E2b92"), null, true),
    assetRegistry.register(1003, "aUSDC", "Erc20", 0, "aUSDC", 6, location("0x2ec4884088D84E5C2970A034732E5209b0aCFA93"), null, true),
    assetRegistry.register(1004, "aWBTC", "Erc20", 0, "aWBTC", 8, location("0x02759D14d0D4F452B9c76f5A230750E8857D36f2"), null, true),
    supplyCall(treasuryAccount, config.DOT, ethers.utils.parseUnits("100000", 10)),
    supplyCall(treasuryAccount, config.WBTC, ethers.utils.parseUnits("17", 8)),
    supplyCall(treasuryAccount, config.USDC, ethers.utils.parseUnits("222222", 6)),
    supplyCall(treasuryAccount, config.USDT, ethers.utils.parseUnits("222222", 6)),
  ];

  const extrinsic = utility.batchAll(batch);
  const batchCallData = extrinsic.method.toHex();

  log(batchCallData);

  log('pause extrinsic');
  log(evmCall({
    from: config.EMERGENCY_ADMIN,
    to: config.POOL_CONFIGURATOR,
    data: pausePoolData(true)
  }).method.toHex());

  log('pause USDT reserve extrinsic');
  log(evmCall({
    from: config.EMERGENCY_ADMIN,
    to: config.POOL_CONFIGURATOR,
    data: reservePauseData(config.DOT, true)
  }).method.toHex());

  await api.disconnect();
  return batchCallData;
}

async function main() {
  try {
    await generateProposal();
    process.exit(0);
  } catch (error) {
    console.error("Error generating batch:", error);
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

module.exports = {
  generateBatchEvmCalls: generateProposal
};
