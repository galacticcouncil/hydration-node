#!/usr/bin/env node
/**
 * update-oracle.js — Send a DIA oracle price update on a local fork.
 *
 * Prerequisites:
 *   The oracleUpdaterAddress must already be overridden to match the sender.
 *   Use override-oracle-signer.js to generate the governance call for that.
 *
 * This script sends setMultipleValues() to the DIA oracle contract via Substrate's
 * evm.call() extrinsic, signed by Alice's Substrate key. The runtime's CallOrigin
 * (EnsureAddressTruncated) maps Alice's Substrate account to her EVM address.
 *
 * Supports multiple --pair/--price pairs in a single call:
 *   node update-oracle.js --pair DOT/USD --price 1.50 --pair AAVE/USD --price 110.1
 *
 * Usage:
 *   node update-oracle.js [--ws ws://127.0.0.1:9944] --pair <PAIR> --price <PRICE> [...]
 *                          [--from 0x...] [--oracle 0x...]
 *
 * Requirements:
 *   npm install @polkadot/api @polkadot/keyring ethers
 */

const { ApiPromise, WsProvider } = require('@polkadot/api');
const { Keyring } = require('@polkadot/keyring');
const { ethers } = require('ethers');

// --- Configuration ---
const ORACLE_CONTRACT_1 = '0xdee629af973ebf5bf261ace12ffd1900ac715f5e';

// Alice's EVM address (truncated from her sr25519 public key)
const ALICE_EVM = '0xd43593c715fdd31c61141abd04a99fd6822c8558';

// DIA DIAOracleV2 ABI fragment
const ORACLE_ABI = [
  'function setMultipleValues(string[] memory keys, uint256[] memory compressedValues)',
  'function getValue(string memory key) view returns (uint128 value, uint128 timestamp)',
];

function parseArgs() {
  const args = process.argv.slice(2);
  const opts = {
    ws: 'ws://127.0.0.1:9944',
    updates: [],  // [{pair, price}]
    oracle: ORACLE_CONTRACT_1,
    from: ALICE_EVM,
  };

  let pendingPair = null;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--ws' && args[i + 1]) opts.ws = args[++i];
    else if (args[i] === '--oracle' && args[i + 1]) opts.oracle = args[++i];
    else if (args[i] === '--from' && args[i + 1]) opts.from = args[++i];
    else if (args[i] === '--pair' && args[i + 1]) {
      pendingPair = args[++i];
    }
    else if (args[i] === '--price' && args[i + 1]) {
      const price = parseFloat(args[++i]);
      if (pendingPair) {
        opts.updates.push({ pair: pendingPair, price });
        pendingPair = null;
      } else {
        console.error('ERROR: --price without preceding --pair');
        process.exit(1);
      }
    }
  }

  if (pendingPair) {
    console.error(`ERROR: --pair ${pendingPair} without --price`);
    process.exit(1);
  }

  if (opts.updates.length === 0) {
    opts.updates.push({ pair: 'DOT/USD', price: 1.50 });
  }

  return opts;
}

/**
 * Pack a DIA oracle price: upper 128 bits = price (8 decimals), lower 128 bits = timestamp.
 */
function packOraclePrice(price, timestamp) {
  const priceScaled = BigInt(Math.round(price * 1e8));
  const ts = BigInt(timestamp);
  return (priceScaled << 128n) | ts;
}

async function main() {
  const opts = parseArgs();
  console.log('=== DIA Oracle Price Update ===');
  console.log(`WS endpoint: ${opts.ws}`);
  console.log(`Oracle contract: ${opts.oracle}`);
  console.log(`Sender EVM: ${opts.from}`);
  console.log(`Updates: ${opts.updates.length}`);
  for (const u of opts.updates) {
    console.log(`  ${u.pair} -> $${u.price}`);
  }
  console.log();

  // Connect via ethers for EVM reads
  const httpUrl = opts.ws.replace('ws://', 'http://').replace('wss://', 'https://');
  const ethProvider = new ethers.JsonRpcProvider(httpUrl);

  // Connect via polkadot.js for substrate extrinsics
  const wsProvider = new WsProvider(opts.ws);
  const api = await ApiPromise.create({ provider: wsProvider });
  const chain = (await api.rpc.system.chain()).toString();
  console.log(`Connected to: ${chain}`);

  // 1. Verify current oracleUpdaterAddress
  const currentUpdater = await ethProvider.getStorage(opts.oracle, 1);
  const currentAddr = '0x' + currentUpdater.slice(-40);
  console.log(`Current oracleUpdaterAddress: ${currentAddr}`);

  if (currentAddr.toLowerCase() !== opts.from.toLowerCase()) {
    console.error(`\nERROR: oracleUpdaterAddress (${currentAddr}) does not match sender (${opts.from})`);
    console.error('Run override-oracle-signer.js first and approve the governance proposal.');
    await api.disconnect();
    process.exit(1);
  }
  console.log('Sender matches oracleUpdaterAddress — OK\n');

  // 2. Read current oracle prices
  const oracle = new ethers.Contract(opts.oracle, ORACLE_ABI, ethProvider);
  for (const u of opts.updates) {
    try {
      const [oldValue, oldTs] = await oracle.getValue(u.pair);
      console.log(`Current ${u.pair}: price=$${Number(oldValue) / 1e8}, timestamp=${oldTs}`);
    } catch (err) {
      console.log(`Could not read current ${u.pair}: ${err.message}`);
    }
  }

  // 3. Build calldata for setMultipleValues()
  const timestamp = Math.floor(Date.now() / 1000);
  const pairs = [];
  const packedValues = [];

  for (const u of opts.updates) {
    const packed = packOraclePrice(u.price, timestamp);
    pairs.push(u.pair);
    packedValues.push(packed);
    console.log(`\n${u.pair}: packed=${packed.toString()} (price=${u.price}, ts=${timestamp})`);
  }

  const iface = new ethers.Interface(ORACLE_ABI);
  const calldata = iface.encodeFunctionData('setMultipleValues', [pairs, packedValues]);

  // 4. Submit via evm.call() signed by Alice
  //    The runtime's CallOrigin (EnsureAddressTruncated) maps Alice's Substrate
  //    account to her EVM address, so this acts as if Alice's EVM address called the contract.
  const keyring = new Keyring({ type: 'sr25519' });
  const alice = keyring.addFromUri('//Alice');

  const gasLimit = 500000;
  const maxFeePerGas = 25000000000; // 25 gwei
  const maxPriorityFeePerGas = 0;

  console.log('\nSubmitting evm.call() signed by Alice...');
  console.log(`  source: ${opts.from}`);
  console.log(`  target: ${opts.oracle}`);
  console.log(`  gas_limit: ${gasLimit}`);

  const evmCall = api.tx.evm.call(
    opts.from,       // source (H160)
    opts.oracle,     // target (H160)
    calldata,        // input (Bytes)
    0,               // value (U256)
    gasLimit,        // gas_limit (u64)
    maxFeePerGas,    // max_fee_per_gas (U256)
    maxPriorityFeePerGas, // max_priority_fee_per_gas (Option<U256>)
    null,            // nonce (Option<U256>) — let the runtime pick
    [],              // access_list (Vec<(H160,Vec<H256>)>)
    [],              // authorization_list (EIP-7702, Vec<AuthorizationListItem>)
  );

  await new Promise((resolve, reject) => {
    evmCall.signAndSend(alice, ({ status, events, dispatchError }) => {
      if (status.isInBlock) {
        console.log(`  Included in block: ${status.asInBlock.toHex()}`);

        if (dispatchError) {
          if (dispatchError.isModule) {
            const decoded = api.registry.findMetaError(dispatchError.asModule);
            console.error(`  Dispatch ERROR: ${decoded.section}.${decoded.name}: ${decoded.docs.join(' ')}`);
          } else {
            console.error(`  Dispatch ERROR: ${dispatchError.toString()}`);
          }
          reject(new Error('evm.call dispatch failed'));
          return;
        }

        // Check for EVM execution result in events
        for (const { event } of events) {
          if (api.events.evm.Executed && api.events.evm.Executed.is(event)) {
            console.log('  EVM Executed successfully');
          }
          if (api.events.evm.ExecutedFailed && api.events.evm.ExecutedFailed.is(event)) {
            console.error('  EVM ExecutedFailed!');
          }
        }
        resolve();
      }
    }).catch(reject);
  });

  // 5. Verify updated prices
  console.log('\nVerifying oracle prices...');
  // Wait a moment for the state to be queryable
  await new Promise(r => setTimeout(r, 2000));
  for (const u of opts.updates) {
    try {
      const [newValue, newTs] = await oracle.getValue(u.pair);
      const priceDecimal = Number(newValue) / 1e8;
      const ok = Math.abs(priceDecimal - u.price) < 0.001;
      console.log(`  ${u.pair}: price=$${priceDecimal}, timestamp=${newTs} ${ok ? '(OK)' : `(MISMATCH — expected ${u.price})`}`);
    } catch (err) {
      console.log(`  ${u.pair}: failed to read — ${err.message}`);
    }
  }

  await api.disconnect();
  console.log('\nDone.');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
