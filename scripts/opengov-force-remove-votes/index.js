const { ApiPromise, Keyring, WsProvider } = require("@polkadot/api");
const { findUnknownExtensions } = require("@polkadot/types/extrinsic/signedExtensions");
const { cryptoWaitReady, encodeAddress } = require("@polkadot/util-crypto");
const readline = require("readline");

const RPC = process.env.RPC_SERVER || "ws://127.0.0.1:9944";
const BATCH_SIZE = Number(process.env.BATCH_SIZE || 20);
const LIMIT = process.env.LIMIT ? Number(process.env.LIMIT) : undefined;
const TX_TIMEOUT_MS = Number(process.env.TX_TIMEOUT_MS || 60_000);
const SUBMIT = process.argv[2] === "submit";
const ALLOWED_NO_EFFECT_EXTENSIONS = new Set(["ValidateClaim", "StorageWeightReclaim"]);

const hdxAddress = (pubKey) => encodeAddress(pubKey, 63);
const chunkify = (items, size) =>
  Array(Math.ceil(items.length / size))
    .fill()
    .map((_, i) => items.slice(i * size, i * size + size));

function assertConfig() {
  if (!Number.isInteger(BATCH_SIZE) || BATCH_SIZE <= 0) {
    throw new Error(`BATCH_SIZE must be a positive integer, got: ${process.env.BATCH_SIZE}`);
  }

  if (LIMIT !== undefined && (!Number.isInteger(LIMIT) || LIMIT <= 0)) {
    throw new Error(`LIMIT must be a positive integer, got: ${process.env.LIMIT}`);
  }

  if (!Number.isInteger(TX_TIMEOUT_MS) || TX_TIMEOUT_MS <= 0) {
    throw new Error(`TX_TIMEOUT_MS must be a positive integer, got: ${process.env.TX_TIMEOUT_MS}`);
  }
}

async function promptSecret(prompt) {
  if (!process.stdin.isTTY) {
    throw new Error("submit mode requires an interactive TTY to prompt for ACCOUNT_SECRET");
  }

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  return new Promise((resolve) => {
    const stdin = process.stdin;
    const onData = (char) => {
      char = char.toString();
      switch (char) {
        case "\n":
        case "\r":
        case "\u0004":
          stdin.removeListener("data", onData);
          break;
        default:
          readline.moveCursor(process.stdout, -rl.line.length, 0);
          readline.clearLine(process.stdout, 1);
          process.stdout.write("*".repeat(rl.line.length));
          break;
      }
    };

    stdin.on("data", onData);
    rl.question(prompt, (value) => {
      rl.close();
      process.stdout.write("\n");
      resolve(value);
    });
  });
}

function isOngoingReferendum(info) {
  if (info.isNone) {
    return false;
  }

  const unwrapped = info.unwrap();
  return unwrapped.isOngoing;
}

function castingVotes(voting) {
  if (!voting.isCasting) {
    return [];
  }

  return voting.asCasting.votes.map(([refIndex]) => refIndex.toNumber());
}

async function votingStats(api) {
  const entries = await api.query.convictionVoting.votingFor.entries();
  let castingRecords = 0;
  let delegatingRecords = 0;
  let votes = 0;

  for (const [, voting] of entries) {
    if (voting.isCasting) {
      castingRecords += 1;
      votes += castingVotes(voting).length;
    } else if (voting.isDelegating) {
      delegatingRecords += 1;
    }
  }

  return {
    votingForRecords: entries.length,
    castingRecords,
    delegatingRecords,
    votes,
  };
}

function logVotingStats(label, stats) {
  console.log(
    `${label} VotingFor stats - records: ${stats.votingForRecords}, casting: ${stats.castingRecords}, delegating: ${stats.delegatingRecords}, votes: ${stats.votes}`
  );
}

function assertSignedExtensionsSupported(api) {
  const unknown = findUnknownExtensions(api.registry.signedExtensions);
  const unsupported = unknown.filter((name) => !ALLOWED_NO_EFFECT_EXTENSIONS.has(name));

  if (unknown.length > 0) {
    console.log(`unknown no-effect signed extensions allowed: ${unknown.join(", ")}`);
  }

  if (unsupported.length === 0) {
    return;
  }

  throw new Error(
    [
      `Unsupported signed extensions: ${unsupported.join(", ")}`,
      "Your @polkadot/api version cannot safely sign transactions for this runtime.",
      "Update dependencies before submitting:",
      "  rm -rf node_modules package-lock.json",
      "  npm install",
    ].join("\n")
  );
}

async function submitAndWait(api, signer, tx, nonce) {
  return new Promise((resolve, reject) => {
    let unsub;
    let done = false;

    const finish = (fn, value) => {
      if (done) {
        return;
      }

      done = true;
      if (unsub) {
        unsub();
      }
      fn(value);
    };

    tx.signAndSend(signer, { nonce }, (receipt) => {
      if (receipt.status.isInBlock || receipt.status.isFinalized) {
        let failed = false;
        let batchItemFailures = 0;
        receipt.events.forEach(({ event }) => {
          if (api.events.system.ExtrinsicFailed.is(event)) {
            failed = true;
            const [dispatchError] = event.data;
            if (dispatchError.isModule) {
              const decoded = api.registry.findMetaError(dispatchError.asModule);
              console.error(
                `extrinsic failed: ${decoded.section}.${decoded.name}: ${decoded.docs.join(" ")}`
              );
            } else {
              console.error(`extrinsic failed: ${dispatchError.toString()}`);
            }
          }

          if (api.events.utility?.BatchInterrupted?.is(event)) {
            batchItemFailures += 1;
            const [index, dispatchError] = event.data;
            console.error(`batch interrupted at item ${index.toString()}: ${dispatchError.toString()}`);
          }

          if (api.events.utility?.ItemFailed?.is(event)) {
            batchItemFailures += 1;
            const [dispatchError] = event.data;
            console.error(`batch item failed: ${dispatchError.toString()}`);
          }
        });

        if (failed) {
          finish(reject, new Error(`extrinsic ${tx.hash.toHex()} failed`));
        } else {
          finish(resolve, { receipt, batchItemFailures });
        }
      }
    })
      .then((unsubscribe) => {
        unsub = unsubscribe;
      })
      .catch((error) => finish(reject, error));
  });
}

async function submitAndWaitWithTimeout(api, signer, tx, nonce) {
  let timeoutId;
  const timeout = new Promise((_, reject) => {
    timeoutId = setTimeout(
      () => reject(new Error(`transaction ${tx.hash.toHex()} was not included within ${TX_TIMEOUT_MS}ms`)),
      TX_TIMEOUT_MS
    );
  });

  try {
    return await Promise.race([submitAndWait(api, signer, tx, nonce), timeout]);
  } finally {
    clearTimeout(timeoutId);
  }
}

async function nextAccountNonce(api, address) {
  return api.rpc.system.accountNextIndex(address).then((n) => n.toNumber());
}

async function submitBatch(api, signer, batch, batchNumber, totalBatches, nonce) {
  const txHash = batch.tx.hash.toHex();

  console.log(`submitting batch ${batchNumber}/${totalBatches} (${batch.calls.length} calls)`);
  console.log(`batch ${batchNumber}/${totalBatches} tx hash: ${txHash}`);

  try {
    const result = await submitAndWaitWithTimeout(api, signer, batch.tx, nonce);
    return { ...result, included: true, nonce: nonce + 1 };
  } catch (error) {
    const currentNonce = await nextAccountNonce(api, signer.address);
    if (currentNonce > nonce) {
      console.warn(
        `batch ${batchNumber}/${totalBatches}: nonce advanced from ${nonce} to ${currentNonce}; treating batch as included`
      );
      return { batchItemFailures: "unknown", included: true, nonce: currentNonce };
    }

    console.warn(
      `batch ${batchNumber}/${totalBatches}: ${error.message || error}; nonce is still ${currentNonce}, skipping this batch`
    );
    console.warn(`skipped batch first call: ${JSON.stringify(batch.calls[0], (key, value) => (key === "tx" ? undefined : value))}`);
    return { batchItemFailures: "skipped", included: false, nonce: currentNonce };
  }
}

async function buildForceRemoveCalls(api) {
  const entries = await api.query.convictionVoting.votingFor.entries();

  const referendumStatusCache = new Map();
  const calls = [];
  let castingRecords = 0;
  let delegatingRecords = 0;
  let totalVotes = 0;
  let ongoingVotes = 0;
  let missingOrFinishedVotes = 0;

  for (const [key, voting] of entries) {
    const [target, classId] = key.args;

    if (voting.isDelegating) {
      delegatingRecords += 1;
      continue;
    }

    if (!voting.isCasting) {
      continue;
    }

    castingRecords += 1;
    const votes = castingVotes(voting);
    totalVotes += votes.length;

    for (const refIndex of votes) {
      let ongoing = referendumStatusCache.get(refIndex);
      if (ongoing === undefined) {
        const info = await api.query.referenda.referendumInfoFor(refIndex);
        ongoing = isOngoingReferendum(info);
        referendumStatusCache.set(refIndex, ongoing);
      }

      if (ongoing) {
        ongoingVotes += 1;
        continue;
      }

      missingOrFinishedVotes += 1;
      const targetAddress = target.toString();
      calls.push({
        target: targetAddress,
        classId: classId.toString(),
        refIndex,
        tx: api.tx.convictionVoting.forceRemoveVote(targetAddress, classId, refIndex),
      });

      if (LIMIT !== undefined && calls.length >= LIMIT) {
        break;
      }
    }

    if (LIMIT !== undefined && calls.length >= LIMIT) {
      break;
    }
  }

  console.log(`casting records: ${castingRecords}`);
  console.log(`delegating records: ${delegatingRecords}`);
  console.log(`votes in casting records scanned: ${totalVotes}`);
  console.log(`ongoing votes skipped: ${ongoingVotes}`);
  console.log(`finished/missing votes selected: ${missingOrFinishedVotes}`);
  console.log(`forceRemoveVote calls built: ${calls.length}`);

  return calls;
}

async function main() {
  assertConfig();
  await cryptoWaitReady();

  const provider = new WsProvider(RPC);
  const api = await ApiPromise.create({ provider });

  const [chain, nodeVersion] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.version(),
  ]);

  console.log(`connected to ${RPC} (${chain} ${nodeVersion})`);
  console.log(`mode: ${SUBMIT ? "submit" : "dry-run"}`);
  console.log(`tx timeout: ${TX_TIMEOUT_MS}ms`);

  assertSignedExtensionsSupported(api);

  if (!api.tx.convictionVoting.forceRemoveVote) {
    throw new Error("convictionVoting.forceRemoveVote is not available in chain metadata");
  }

  const beforeStats = await votingStats(api);
  logVotingStats("before", beforeStats);

  const calls = await buildForceRemoveCalls(api);
  if (calls.length === 0) {
    console.log("nothing to submit");
    await api.disconnect();
    return;
  }

  let signer;
  if (SUBMIT) {
    const accountSecret = await promptSecret("ACCOUNT_SECRET: ");
    const keyring = new Keyring({ type: "sr25519" });
    signer = keyring.addFromUri(accountSecret);
    console.log(`active account: ${hdxAddress(signer.addressRaw)}`);
  }

  const batches = chunkify(calls, BATCH_SIZE).map((chunk) => ({
    calls: chunk,
    tx: api.tx.utility.batch(chunk.map(({ tx }) => tx)),
  }));

  console.log(`batch size: ${BATCH_SIZE}`);
  console.log(`batches: ${batches.length}`);
  console.log("first call:");
  console.log(JSON.stringify(calls[0], (key, value) => (key === "tx" ? undefined : value), 2));
  console.log("encoded first batch:");
  console.log(batches[0].tx.toHex());

  if (!SUBMIT) {
    console.log("dry-run only; pass `submit` to broadcast");
    await api.disconnect();
    return;
  }

  let skippedBatches = 0;
  for (let i = 0; i < batches.length; i += 1) {
    const batch = batches[i];
    const nonce = await nextAccountNonce(api, signer.address);
    const { batchItemFailures, included } = await submitBatch(
      api,
      signer,
      batch,
      i + 1,
      batches.length,
      nonce
    );

    if (included) {
      console.log(`batch ${i + 1}/${batches.length} included; item failures: ${batchItemFailures}`);
    } else {
      skippedBatches += 1;
      console.log(`batch ${i + 1}/${batches.length} skipped`);
    }
  }

  const afterStats = await votingStats(api);
  logVotingStats("after", afterStats);
  console.log(`VotingFor record delta: ${beforeStats.votingForRecords - afterStats.votingForRecords}`);
  console.log(`vote delta: ${beforeStats.votes - afterStats.votes}`);
  console.log(`skipped batches: ${skippedBatches}`);

  await api.disconnect();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
