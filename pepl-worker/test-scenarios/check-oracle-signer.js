#!/usr/bin/env node
/**
 * check-oracle-signer.js — Read the current oracleUpdaterAddress from DIA oracle contracts.
 *
 * Usage:
 *   node check-oracle-signer.js [--rpc http://127.0.0.1:9999]
 */

const { ethers } = require('ethers');

const ORACLE_CONTRACTS = [
  { name: 'Oracle 1', address: '0xdee629af973ebf5bf261ace12ffd1900ac715f5e' },
  { name: 'Oracle 2', address: '0x48ae7803cd09c48434e3fc5629f15fb76f0b5ce5' },
];

const KNOWN_SIGNERS = {
  '0x33a5e905fb83fcfb62b0dd1595dfbc06792e054e': 'Default signer 1',
  '0xff0c624016c873d359dde711b42a2f475a5a07d3': 'Default signer 2',
  '0xd43593c715fdd31c61141abd04a99fd6822c8558': 'Alice (dev)',
};

function parseArgs() {
  const args = process.argv.slice(2);
  let rpc = 'http://127.0.0.1:9999';
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--rpc' && args[i + 1]) rpc = args[++i];
  }
  return { rpc };
}

async function main() {
  const opts = parseArgs();
  const provider = new ethers.JsonRpcProvider(opts.rpc);

  console.log(`RPC: ${opts.rpc}\n`);

  for (const oracle of ORACLE_CONTRACTS) {
    const raw = await provider.getStorage(oracle.address, 1);
    const addr = '0x' + raw.slice(-40);
    const label = KNOWN_SIGNERS[addr.toLowerCase()] || 'unknown';
    console.log(`${oracle.name} (${oracle.address})`);
    console.log(`  oracleUpdaterAddress: ${addr}  [${label}]\n`);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
