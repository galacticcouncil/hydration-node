#!/usr/bin/env node
/**
 * override-oracle-signer.js — Generate the encoded `system.setStorage` call that replaces
 * the DIA oracle contract's oracleUpdaterAddress with a new EVM address.
 *
 * This does NOT submit anything. It outputs the encoded call hex which you then propose
 * via governance (e.g. paste into Polkadot JS → Governance → Referenda → Submit preimage).
 *
 * Must be re-proposed after every chain restart (fresh state restores the original signer).
 *
 * How it works:
 *   The DIA DIAOracleV2 contract stores `oracleUpdaterAddress` in Solidity slot 1.
 *   In Substrate, EVM contract storage lives in `pallet_evm::AccountStorages(H160, H256)`.
 *   `system.setStorage` overwrites the raw storage key for (oracle_contract, slot_1)
 *   with the new EVM address left-padded to H256.
 *
 * After the governance proposal executes:
 *   - The new signer can call setMultipleValues() on the oracle contract
 *   - Start the PEPL worker node with --oracle-update-signer <NEW_EVM> so it also accepts
 *     the new signer's oracle update TXs at the worker level
 *
 * Usage:
 *   node override-oracle-signer.js [--ws ws://127.0.0.1:9944] [--signer 0x...] [--oracle 0x...]
 *
 * Requirements:
 *   npm install @polkadot/api @polkadot/util @polkadot/util-crypto
 */

const { ApiPromise, WsProvider } = require('@polkadot/api');
const { hexToU8a, u8aToHex } = require('@polkadot/util');
const { xxhashAsHex, blake2AsHex } = require('@polkadot/util-crypto');

// --- Default addresses ---

// DIA oracle contracts on mainnet
const ORACLE_CONTRACT_1 = '0xdee629af973ebf5bf261ace12ffd1900ac715f5e';
const ORACLE_CONTRACT_2 = '0x48ae7803cd09c48434e3fc5629f15fb76f0b5ce5';

// oracleUpdaterAddress is at Solidity storage slot 1
const UPDATER_SLOT = '0x0000000000000000000000000000000000000000000000000000000000000001';

// Alice's EVM address (truncated from her sr25519 public key — standard Substrate EVM mapping)
const ALICE_EVM = '0xd43593c715fdd31c61141abd04a99fd6822c8558';

// --- Helpers ---

/**
 * Build Substrate storage key for pallet_evm::AccountStorages(contract, slot).
 *
 * Layout: twox128("EVM") ++ twox128("AccountStorages") ++ blake2_128_concat(H160) ++ blake2_128_concat(H256)
 */
function buildAccountStoragesKey(contractAddress, slot) {
  const palletPrefix = xxhashAsHex('EVM', 128).slice(2);
  const storagePrefix = xxhashAsHex('AccountStorages', 128).slice(2);

  // blake2_128_concat for H160 (20 bytes)
  const contractBytes = hexToU8a(contractAddress);
  const contractHash = blake2AsHex(contractBytes, 128).slice(2);
  const contractConcat = contractHash + u8aToHex(contractBytes).slice(2);

  // blake2_128_concat for H256 (32 bytes)
  const slotBytes = hexToU8a(slot);
  const slotHash = blake2AsHex(slotBytes, 128).slice(2);
  const slotConcat = slotHash + u8aToHex(slotBytes).slice(2);

  return '0x' + palletPrefix + storagePrefix + contractConcat + slotConcat;
}

function parseArgs() {
  const args = process.argv.slice(2);
  const opts = {
    ws: 'ws://127.0.0.1:9944',
    signer: ALICE_EVM,
    oracles: [ORACLE_CONTRACT_1, ORACLE_CONTRACT_2],
  };
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--ws' && args[i + 1]) opts.ws = args[++i];
    else if (args[i] === '--signer' && args[i + 1]) opts.signer = args[++i];
    else if (args[i] === '--oracle' && args[i + 1]) {
      opts.oracles = [args[++i]];
    }
  }
  return opts;
}

async function main() {
  const opts = parseArgs();

  console.log('=== Override DIA Oracle Signer — Encoded Call Generator ===');
  console.log(`WS endpoint: ${opts.ws}`);
  console.log(`New signer:  ${opts.signer}`);
  console.log(`Oracles:     ${opts.oracles.join(', ')}`);

  // Connect (needed to build the encoded call with correct metadata)
  const provider = new WsProvider(opts.ws);
  const api = await ApiPromise.create({ provider });
  const chain = (await api.rpc.system.chain()).toString();
  console.log(`Connected to: ${chain}`);

  // Build storage overrides for all oracle contracts
  const storageItems = [];
  for (const oracle of opts.oracles) {
    const storageKey = buildAccountStoragesKey(oracle, UPDATER_SLOT);
    const newValue = '0x000000000000000000000000' + opts.signer.slice(2).toLowerCase();

    console.log(`\n--- Oracle contract: ${oracle} ---`);
    console.log(`  Storage key: ${storageKey}`);
    console.log(`  New value:   ${newValue}`);

    // Read current value for reference
    const currentRaw = await api.rpc.state.getStorage(storageKey);
    if (currentRaw.isSome || currentRaw.toString() !== '0x') {
      const currentAddr = '0x' + currentRaw.toString().slice(-40);
      console.log(`  Current oracleUpdaterAddress: ${currentAddr}`);
    } else {
      console.log(`  Current oracleUpdaterAddress: (empty/unset)`);
    }

    storageItems.push([storageKey, newValue]);
  }

  // Build the encoded call
  const setStorageCall = api.tx.system.setStorage(storageItems);
  const encodedCall = setStorageCall.method.toHex();

  console.log('\n=== ENCODED CALL ===');
  console.log('');
  console.log('Call: system.setStorage');
  console.log(`Items: ${storageItems.length} storage key-value pair(s)`);
  console.log('');
  console.log('Encoded call data (paste into Polkadot JS → Governance → Submit preimage):');
  console.log('');
  console.log(encodedCall);
  console.log('');
  console.log(`Call hash: ${setStorageCall.method.hash.toHex()}`);
  console.log('');
  console.log('Steps to apply:');
  console.log('  1. Go to Polkadot JS → Developer → Extrinsics → Decode');
  console.log('     Paste the encoded call above to verify it decodes correctly');
  console.log('  2. Go to Governance → Referenda → Submit preimage');
  console.log('     Paste the encoded call hex');
  console.log('  3. Submit referendum on the appropriate track (e.g. Root or WhitelistedCaller)');
  console.log('  4. Vote and wait for execution');
  console.log('');
  console.log('After execution, start the PEPL worker with:');
  console.log(`  --oracle-update-signer ${opts.signer}`);

  await api.disconnect();
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
