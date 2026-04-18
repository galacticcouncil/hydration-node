#!/usr/bin/env node
/**
 * Deploy a runtime upgrade to a Hydration fork/testnet via governance.
 *
 * Flow:
 *   1. Submit referendum on Root track with inline call system.authorizeUpgrade(wasmHash)
 *   2. Place decision deposit
 *   3. Vote aye with a large HDX amount (default 3B)
 *   4. Wait for referendum to be approved and scheduled call to execute
 *   5. Submit unsigned system.applyAuthorizedUpgrade(wasm)
 *   6. Wait for specVersion to change
 *
 * Usage:
 *   RPC=wss://node.hydration.cloud WASM=path/to/runtime.wasm node upgrade.mjs
 *   SURI="//Alice" node upgrade.mjs
 *
 * Env vars:
 *   RPC         — WebSocket endpoint of the target chain (required)
 *   WASM        — path to hydradx_runtime.compact.compressed.wasm (required)
 *   SURI        — signing key URI (default: //Alice)
 *   VOTE_HDX    — amount to vote aye with, in whole HDX (default: 3_000_000_000)
 *   ENACT_AFTER — blocks until enactment after approval (default: 10)
 */
import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';
import { cryptoWaitReady, blake2AsHex } from '@polkadot/util-crypto';
import { u8aToHex } from '@polkadot/util';
import fs from 'fs';

const RPC = process.env.RPC || 'ws://127.0.0.1:9944';
const WASM_PATH = process.env.WASM || './hydradx_runtime.compact.compressed.wasm';
const SURI = process.env.SURI || '//Alice';
const VOTE_HDX = BigInt(process.env.VOTE_HDX || '3000000000');
const ENACT_AFTER = parseInt(process.env.ENACT_AFTER || '10', 10);

const HDX_DECIMALS = 12n;
const VOTE_AMOUNT = VOTE_HDX * 10n ** HDX_DECIMALS;

function decodeDispatchError(api, dispatchError) {
  if (dispatchError.isModule) {
    const d = api.registry.findMetaError(dispatchError.asModule);
    return `${d.section}.${d.name}: ${d.docs.join(' ')}`;
  }
  return dispatchError.toString();
}

async function submitAndWait(api, tx, signer, label) {
  console.log(`→ ${label}`);
  return new Promise((resolve, reject) => {
    tx.signAndSend(signer, ({ status, dispatchError, events }) => {
      if (dispatchError) reject(new Error(decodeDispatchError(api, dispatchError)));
      if (status.isInBlock) {
        console.log(`  in block ${status.asInBlock.toHex()}`);
        resolve({ blockHash: status.asInBlock, events });
      }
    }).catch(reject);
  });
}

async function main() {
  await cryptoWaitReady();
  const api = await ApiPromise.create({ provider: new WsProvider(RPC) });
  console.log(`Connected to ${RPC}`);
  console.log(`Chain: ${await api.rpc.system.chain()}`);

  const version = await api.rpc.state.getRuntimeVersion();
  const startVersion = version.specVersion.toNumber();
  console.log(`Current runtime: ${version.specName.toString()} v${startVersion}`);

  const wasm = fs.readFileSync(WASM_PATH);
  const wasmHex = u8aToHex(wasm);
  const wasmHash = blake2AsHex(wasm, 256);
  console.log(`Target WASM: ${wasm.length} bytes, hash ${wasmHash}`);

  const keyring = new Keyring({ type: 'sr25519' });
  const signer = keyring.addFromUri(SURI);
  console.log(`Signer: ${signer.address}`);

  const bal = await api.query.system.account(signer.address);
  console.log(`Signer balance: ${bal.data.free.toString()}`);

  // 1. Submit referendum on Root track with inline authorizeUpgrade call
  const authorizeCall = api.tx.system.authorizeUpgrade(wasmHash);
  const proposal = { Inline: authorizeCall.method.toHex() };

  const { events: submitEvents } = await submitAndWait(
    api,
    api.tx.referenda.submit({ system: 'Root' }, proposal, { After: ENACT_AFTER }),
    signer,
    'Submit referendum on Root track'
  );

  let refIndex = null;
  for (const { event } of submitEvents) {
    if (event.section === 'referenda' && event.method === 'Submitted') {
      refIndex = event.data[0].toNumber();
      break;
    }
  }
  if (refIndex === null) throw new Error('No Submitted event in referendum submission');
  console.log(`Referendum index: ${refIndex}`);

  // 2. Place decision deposit
  await submitAndWait(
    api,
    api.tx.referenda.placeDecisionDeposit(refIndex),
    signer,
    `Place decision deposit for referendum ${refIndex}`
  );

  // 3. Vote aye
  await submitAndWait(
    api,
    api.tx.convictionVoting.vote(refIndex, {
      Standard: {
        vote: { aye: true, conviction: 'None' },
        balance: VOTE_AMOUNT,
      },
    }),
    signer,
    `Vote aye with ${VOTE_HDX} HDX`
  );

  // 4. Wait for referendum approved, then for authorize call to execute
  console.log('Waiting for referendum to pass...');
  let approved = false;
  for (let i = 0; i < 60; i++) {
    const info = await api.query.referenda.referendumInfoFor(refIndex);
    const h = info.toHuman();
    if (h?.Approved) {
      console.log(`Referendum approved at block ${h.Approved[0]}`);
      approved = true;
      break;
    }
    if (h?.Rejected || h?.Cancelled || h?.TimedOut || h?.Killed) {
      throw new Error(`Referendum ended: ${JSON.stringify(h)}`);
    }
    const head = await api.rpc.chain.getHeader();
    const state = h?.Ongoing?.deciding ? 'deciding' : (h?.Ongoing ? 'preparing' : 'unknown');
    console.log(`  block #${head.number.toNumber()} — ${state}`);
    await new Promise((r) => setTimeout(r, 6000));
  }
  if (!approved) throw new Error('Referendum did not pass in time');

  console.log('Waiting for authorize call to execute...');
  let auth;
  for (let i = 0; i < 30; i++) {
    auth = await api.query.system.authorizedUpgrade();
    const head = await api.rpc.chain.getHeader();
    if (auth.isSome) {
      console.log(`  block #${head.number.toNumber()}: authorized ${auth.unwrap().codeHash.toHex()}`);
      break;
    }
    console.log(`  block #${head.number.toNumber()}: not yet authorized`);
    await new Promise((r) => setTimeout(r, 6000));
  }
  if (auth.isNone) throw new Error('Upgrade not authorized after waiting');

  // 5. Apply via unsigned extrinsic
  console.log('Applying authorized upgrade...');
  await new Promise((resolve, reject) => {
    api.tx.system.applyAuthorizedUpgrade(wasmHex).send(({ status, dispatchError }) => {
      if (dispatchError) reject(new Error(decodeDispatchError(api, dispatchError)));
      if (status.isInBlock) {
        console.log(`  applyAuthorizedUpgrade in block ${status.asInBlock.toHex()}`);
        resolve();
      }
    }).catch(reject);
  });

  // 6. Wait for specVersion to bump
  console.log('Waiting for runtime upgrade to take effect...');
  for (let i = 0; i < 30; i++) {
    await new Promise((r) => setTimeout(r, 6000));
    const v = await api.rpc.state.getRuntimeVersion();
    const head = await api.rpc.chain.getHeader();
    const cur = v.specVersion.toNumber();
    console.log(`  block #${head.number.toNumber()}: specVersion ${cur}`);
    if (cur !== startVersion) {
      console.log(`\n✓ Runtime upgraded from v${startVersion} to v${cur}`);
      await api.disconnect();
      return;
    }
  }
  throw new Error('specVersion did not change within timeout');
}

main().catch((e) => {
  console.error('FAIL:', e.message || e);
  process.exit(1);
});
