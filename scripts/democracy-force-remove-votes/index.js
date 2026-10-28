const fs = require("node:fs");
const { ApiPromise, Keyring, WsProvider } = require("@polkadot/api");
const { findUnknownExtensions } = require("@polkadot/types/extrinsic/signedExtensions");
const { cryptoWaitReady, encodeAddress, xxhashAsHex } = require("@polkadot/util-crypto");
const { planCleanup, batchCalls } = require("./planner");
const { promptSecret } = require("./secret");
const { submitAndFinalize } = require("./submit");

const DEMOCRACY_LOCK = "0x64656d6f63726163";

function positiveInteger(name, fallback) {
  const value = process.env[name] === undefined ? fallback : Number(process.env[name]);
  if (value !== undefined && (!Number.isSafeInteger(value) || value <= 0)) {
    throw new Error(`${name} must be a positive safe integer`);
  }
  return value;
}

async function countDemocracyKeys(api, hash) {
  const prefix = xxhashAsHex("Democracy", 128);
  let start = prefix;
  let count = 0;
  for (;;) {
    const keys = await api.rpc.state.getKeysPaged(prefix, 1000, start, hash);
    count += keys.length;
    if (keys.length < 1000) return count;
    start = keys[keys.length - 1].toHex();
  }
}

async function snapshot(api) {
  const hash = await api.rpc.chain.getFinalizedHead();
  const state = await api.at(hash);
  if (!state.query.democracy?.votingOf) {
    throw new Error("Democracy is absent at the finalized head; run this before the removal upgrade");
  }
  const [entries, positions, processed, deposits, democracyKeys, header] = await Promise.all([
    state.query.democracy.votingOf.entries(),
    state.query.staking.positionVotes.entries(),
    state.query.staking.processedVotes.keys(),
    state.query.democracy.depositOf.keys(),
    countDemocracyKeys(api, hash),
    api.rpc.chain.getHeader(hash),
  ]);
  const lockedAccounts = new Set();
  for (let offset = 0; offset < entries.length; offset += 256) {
    const targets = entries.slice(offset, offset + 256).map(([key]) => key.args[0].toString());
    const locks = await state.query.balances.locks.multi(targets);
    locks.forEach((list, index) => {
      if (list.some((lock) => lock.id.toHex() === DEMOCRACY_LOCK)) lockedAccounts.add(targets[index]);
    });
  }
  const direct = entries.filter(([, voting]) => voting.isDirect);
  const stats = {
    block: header.number.toString(),
    hash: hash.toHex(),
    democracyStorageKeys: democracyKeys,
    votingRecords: entries.length,
    directRecords: direct.length,
    delegatingRecords: entries.filter(([, voting]) => voting.isDelegating).length,
    votes: direct.reduce((sum, [, voting]) => sum + voting.asDirect.votes.length, 0),
    accountsWithDemocracyLocks: lockedAccounts.size,
    depositRecords: deposits.length,
    legacyStakingPositionRecords: positions.length,
    legacyStakingVotes: positions.reduce((sum, [, voting]) => sum + voting.votes.length, 0),
    legacyProcessedVoteRecords: processed.length,
  };
  return { hash, state, entries, lockedAccounts, stats };
}

async function checkBatchWeight(api, batch, signer) {
  const info = await batch.tx.paymentInfo(signer);
  const maximum = api.consts.system.blockWeights.perClass.normal.maxExtrinsic;
  if (maximum.isNone) throw new Error("No normal maxExtrinsic weight in runtime configuration");
  const bound = maximum.unwrap();
  for (const dimension of ["refTime", "proofSize"]) {
    if (BigInt(info.weight[dimension].toString()) > BigInt(bound[dimension].toString())) {
      throw new Error(`Batch exceeds ${dimension} limit; reduce BATCH_SIZE`);
    }
  }
}

async function main() {
  const mode = process.argv[2] || "dry-run";
  if (!["dry-run", "submit"].includes(mode)) throw new Error("Usage: node index.js [dry-run|submit]");
  const batchSize = positiveInteger("BATCH_SIZE", 20);
  const limit = positiveInteger("LIMIT");
  const timeout = positiveInteger("TX_TIMEOUT_MS", 180_000);
  const rpc = process.env.RPC_SERVER || "ws://127.0.0.1:9944";
  await cryptoWaitReady();
  const api = await ApiPromise.create({ provider: new WsProvider(rpc) });
  try {
    if (!api.tx.democracy?.forceRemoveVote || !api.tx.democracy?.unlock || !api.tx.utility?.batchAll) {
      throw new Error("Required democracy.forceRemoveVote, democracy.unlock or utility.batchAll is absent");
    }
    const specVersion = api.runtimeVersion.specVersion.toString();
    const before = await snapshot(api);
    console.log(`Connected to ${rpc}; mode: ${mode}; runtime: ${specVersion}`);
    console.log("Before:", JSON.stringify(before.stats, null, 2));
    const plan = await planCleanup(
      before.entries, before.lockedAccounts,
      (index) => before.state.query.democracy.referendumInfoOf(index), limit
    );
    const batches = batchCalls(api, plan.calls, batchSize);
    console.log(JSON.stringify({
      selectedVotes: plan.selectedVotes, ongoingVotesSkipped: plan.ongoingVotes,
      votesDeferredByLimit: plan.deferredVotes, delegatingRecordsSkipped: plan.delegatingRecords,
      unlockCalls: plan.calls.filter((call) => call.kind === "unlock").length,
      batches: batches.length,
    }, null, 2));
    const output = process.env.OUTPUT_FILE || "cleanup-plan.json";
    fs.writeFileSync(output, JSON.stringify({
      rpc, genesisHash: api.genesisHash.toHex(), specVersion, snapshot: before.stats,
      batches: batches.map((batch) => ({ calls: batch.calls, callHex: batch.tx.method.toHex() })),
    }, null, 2) + "\n");
    console.log(`Wrote encoded batches to ${output}`);
    if (mode !== "submit" || !batches.length) return;

    const unsupported = findUnknownExtensions(api.registry.signedExtensions)
      .filter((name) => !["ValidateClaim", "StorageWeightReclaim"].includes(name));
    if (unsupported.length) throw new Error(`Unsupported signed extensions: ${unsupported.join(", ")}`);
    const keyring = new Keyring({ type: "sr25519" });
    const signer = keyring.addFromUri(await promptSecret("ACCOUNT_SECRET: "));
    console.log(`Signer: ${encodeAddress(signer.addressRaw, 63)}`);
    const members = await api.query.technicalCommittee.members();
    const signerHex = "0x" + Buffer.from(signer.publicKey).toString("hex");
    if (!members.some((member) => member.toHex() === signerHex)) {
      throw new Error("Signer is not a current technical committee member");
    }
    for (let index = 0; index < batches.length; index += 1) {
      if (api.runtimeVersion.specVersion.toString() !== specVersion) {
        throw new Error("Runtime changed since the scan; stopping for a fresh plan");
      }
      const batch = batches[index];
      await checkBatchWeight(api, batch, signer);
      const nonce = await api.rpc.system.accountNextIndex(signer.address);
      console.log(`Submitting batch ${index + 1}/${batches.length} (${batch.calls.length} calls)`);
      const result = await submitAndFinalize(api, signer, batch.tx, nonce, timeout);
      console.log(`Finalized in ${result.status.asFinalized.toHex()}`);
    }
    console.log("After:", JSON.stringify((await snapshot(api)).stats, null, 2));
  } finally {
    await api.disconnect();
  }
}

main().catch((error) => {
  console.error(error.message || error);
  process.exitCode = 1;
});
