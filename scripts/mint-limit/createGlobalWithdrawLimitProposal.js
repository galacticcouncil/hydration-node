import { ApiPromise, WsProvider } from '@polkadot/api';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';
import readline from 'node:readline';

const RPC = process.env.RPC || 'wss://rpc.hydradx.cloud';
const IS_TESTNET = /^(1|true|yes)$/i.test(process.env.IS_TESTNET || '');
const DEFAULT_TC_THRESHOLD = IS_TESTNET ? 1 : 4;
const TC_THRESHOLD = Number(process.env.TC_THRESHOLD || String(DEFAULT_TC_THRESHOLD));
const SUBMIT = /^(1|true|yes)$/i.test(process.env.SUBMIT || '');
const ENV_SIGNER_URI = process.env.SIGNER_URI || '';
const HDX_DECIMALS = 12n;
const HDX_UNIT = 10n ** HDX_DECIMALS;
const DEFAULT_GLOBAL_LIMIT_HDX_UNITS = IS_TESTNET ? 1_000_000n * HDX_UNIT : 1_000_000_000n * HDX_UNIT;
const GLOBAL_LIMIT_HDX_UNITS = process.env.GLOBAL_LIMIT_HDX_UNITS || DEFAULT_GLOBAL_LIMIT_HDX_UNITS.toString();
const WINDOW_MS = IS_TESTNET ? 1_800_000 : 21_600_000; // 30m or 6h

const EGRESS_ACCOUNTS = [
  // Parachain sovereign accounts
  '7LCt6dFqtxzdKVB2648jWW9d85doiFfLSbZJDNAMVJNxh5rJ', // Asset Hub Polkadot (1000)
  '7LCt6dFrhTDMPxtFNa3eHEhuWcHUHhPpqSSajnpbrUWqbhJ1', // People Polkadot (1004)
  '7LCt6dFm6ED8s6CoGTKmdmwktSDbwbbNNr7dd74ruE21MgAz', // Acala (2000)
  '7LCt6dFmtiRrwZv2YyEgQWW3GxsGX3Krmgzv9Xj7GQ9tG2j8', // Moonbeam (2004)
  '7LCt6dFnHxYDyomeCEC8nsnBUEC6omC6y7SZQk4ESzDpiDYo', // Astar (2006)
  '7LCt6dFs6sraSg31uKfbRH7soQ66GRb3LAkGZJ1ie3369crq', // Bifrost (2030)
  '7LCt6dFsW7xwUutdYad3oeQ1zfQvZ9THXbBupWLqpd72bmnM', // Interlay (2032)
  '7LCt6dFCdZxUErjoxSYYaygmgR37WdWHW3Gc1xaL1tdYCXAw', // Pendulum (2094)
  '7LCt6dFuhxZwCE6WaVt3vfRo4chW97idcQcwmBA2KrUh6QXS', // Neuroweb (2043)
  '7LCt6dF6pmXngyxLka1ZwFa1UzRmwfrT24gqujsXNbDKFVir', // Energy Web X (3345)
  '7LCt6dFBdgr99rDiTfV2ZeuhpAKmQLFPP7zZ4Hq1Ze2agqBW', // Mythos (3369)
];

const LOCAL_ASSETS = [
  { id: 0, label: 'HDX' },
  { id: 222, label: 'HOLLAR' },
];

const EXTERNAL_ASSETS = [
  2, 3, 4, 5, 6, 7, 9, 10, 11, 13, 14, 15, 16, 18, 19, 20, 21, 22, 23,
  30, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 252525, 1000081,
  1000085, 1000099, 1000100, 1000189, 1000190, 1000198, 1000624, 1000625,
  1000626, 1000745, 1000746, 1000752, 1000753, 1000765, 1000766, 1000767,
  1000771, 1000794, 1000795, 1000796, 1000809, 1000851,
];

function buildTechnicalCommitteePropose(api, call, threshold) {
  const len = call.method.encodedLength ?? call.method.toU8a().length;
  return api.tx.technicalCommittee.propose(threshold, call.method, len);
}

async function promptHidden(query) {
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    terminal: true,
  });

  return await new Promise((resolve) => {
    rl.question(query, { hideEchoBack: true }, (answer) => {
      rl.close();
      resolve(answer.trim());
    });
  });
}

async function getSignerUri() {
  if (ENV_SIGNER_URI) {
    return ENV_SIGNER_URI;
  }

  if (!SUBMIT) {
    return '';
  }

  return await promptHidden('Enter SIGNER_URI (hidden): ');
}

async function submitCall(call, signerUri) {
  if (!SUBMIT) {
    return;
  }

  if (!signerUri) {
    throw new Error('SUBMIT=true requires SIGNER_URI to be set.');
  }

  await cryptoWaitReady();
  const keyring = new Keyring({ type: 'sr25519' });
  const signer = keyring.addFromUri(signerUri);

  console.log(`\n--- Submitting as ---\n${signer.address}`);

  await new Promise((resolve, reject) => {
    let unsub = null;

    call.signAndSend(signer, (result) => {
      if (result.status.isInBlock) {
        console.log(`\n--- Included in block ---\n${result.status.asInBlock.toHex()}`);
      }

      if (result.status.isFinalized) {
        console.log(`\n--- Finalized in block ---\n${result.status.asFinalized.toHex()}`);
        console.log(`\n--- Extrinsic hash ---\n${call.hash.toHex()}`);
        if (unsub) unsub();
        resolve();
      }

      if (result.isError) {
        if (unsub) unsub();
        reject(new Error('Extrinsic failed.'));
      }
    })
      .then((u) => {
        unsub = u;
      })
      .catch(reject);
  });
}

async function main() {
  const signerUri = await getSignerUri();
  const provider = new WsProvider(RPC);
  const api = await ApiPromise.create({ provider, noInitWarn: true });

  const calls = [];

  calls.push(
    api.tx.circuitBreaker.setGlobalWithdrawLimitParams({
      limit: GLOBAL_LIMIT_HDX_UNITS,
      window: WINDOW_MS,
    })
  );

  calls.push(api.tx.circuitBreaker.addEgressAccounts(EGRESS_ACCOUNTS));

  for (const asset of LOCAL_ASSETS) {
    calls.push(api.tx.circuitBreaker.setAssetCategory(asset.id, 'Local'));
  }

  for (const assetId of EXTERNAL_ASSETS) {
    calls.push(api.tx.circuitBreaker.setAssetCategory(assetId, 'External'));
  }

  const batch = api.tx.utility.batchAll(calls);
  const preimage = api.tx.preimage.notePreimage(batch.method.toHex());
  const tcProposal = buildTechnicalCommitteePropose(api, batch, TC_THRESHOLD);
  const lengthBound = batch.method.encodedLength ?? batch.method.toU8a().length;

  console.log('--- Config summary ---');
  console.log(`Mode: ${IS_TESTNET ? 'testnet' : 'mainnet'}`);
  console.log(`RPC: ${RPC}`);
  console.log(`TC threshold: ${TC_THRESHOLD}`);
  console.log(`Submit: ${SUBMIT ? 'yes' : 'no'}`);
  console.log(`Global withdraw limit units: ${GLOBAL_LIMIT_HDX_UNITS}`);
  console.log(`Window: ${WINDOW_MS} ms`);
  console.log(`Egress accounts: ${EGRESS_ACCOUNTS.length}`);
  console.log(`Local overrides: ${LOCAL_ASSETS.map((a) => `${a.label}(${a.id})`).join(', ')}`);
  console.log(`External overrides: ${EXTERNAL_ASSETS.length}`);

  console.log('\n--- utility.batchAll (human) ---\n', batch.method.toHuman());
  console.log('\n--- utility.batchAll HEX ---\n', batch.method.toHex());

  console.log('\n--- preimage.notePreimage (human) ---\n', preimage.method.toHuman());
  console.log('\n--- preimage.notePreimage HEX ---\n', preimage.method.toHex());

  console.log('\n--- technicalCommittee.propose (human) ---\n', tcProposal.method.toHuman());
  console.log('\n--- technicalCommittee.propose HEX ---\n', tcProposal.method.toHex());
  console.log('\n--- Length bound ---\n', lengthBound);

  await submitCall(tcProposal, signerUri);

  await api.disconnect();
}

main().catch((e) => {
  console.error('\nERROR:', e.message || e);
  process.exit(1);
});
