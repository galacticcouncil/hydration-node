const { ApiPromise, WsProvider } = require("@polkadot/api");
const { Keyring } = require("@polkadot/keyring");
const { blake2AsHex } = require("@polkadot/util-crypto");

const WS_URL = process.env.WS_URL || "ws://127.0.0.1:9999";

function sendAndWait(tx, signer, api, timeoutMs = 60000) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("Transaction timed out")), timeoutMs);
    tx.signAndSend(signer, ({ status, events, dispatchError }) => {
      if (status.isReady) console.log("    tx: ready");
      if (status.isBroadcast) console.log("    tx: broadcast");
      if (dispatchError) {
        clearTimeout(timer);
        if (dispatchError.isModule) {
          const decoded = api.registry.findMetaError(dispatchError.asModule);
          reject(new Error(`${decoded.section}.${decoded.name}: ${decoded.docs.join(" ")}`));
        } else {
          reject(new Error(dispatchError.toString()));
        }
        return;
      }
      if (status.isInBlock) {
        clearTimeout(timer);
        console.log(`    tx: in block ${status.asInBlock.toString().slice(0, 18)}...`);
        resolve({ blockHash: status.asInBlock, events });
      }
    }).catch((err) => {
      clearTimeout(timer);
      reject(err);
    });
  });
}

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

async function main() {
  console.log("=".repeat(60));
  console.log("  Proxy Fee Test: Chain Setup");
  console.log("  Sets WETH location + accepted currency for EVM fees");
  console.log("=".repeat(60));
  console.log(`\n  RPC: ${WS_URL}`);

  const provider = new WsProvider(WS_URL);
  const api = await ApiPromise.create({ provider });

  const [chain, nodeName] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.name(),
  ]);
  console.log(`  Connected to: ${chain} (${nodeName})\n`);

  const keyring = new Keyring({ type: "sr25519" });
  const alice = keyring.addFromUri("//Alice");

  const wethLoc = {
    parents: 1,
    interior: {
      X3: [
        { Parachain: 2004 },
        { PalletInstance: 110 },
        { AccountKey20: { key: "0xab3f0245b83feb11d15aaffefd7ad465a59817ed" } },
      ],
    },
  };

  const wethByLocation = await api.query.assetRegistry.locationAssets(wethLoc);
  const wethId = wethByLocation.isSome ? wethByLocation.toJSON() : null;
  const wethAccepted = wethId ? await api.query.multiTransactionPayment.acceptedCurrencies(wethId) : null;

  console.log("  WETH asset ID:  " + (wethId || "NOT REGISTERED"));
  console.log("  WETH accepted:  " + (wethAccepted && wethAccepted.isSome ? "set" : "NOT SET"));

  if (wethId && wethAccepted && wethAccepted.isSome) {
    console.log("\n  Chain is already configured. Skipping setup.\n");
    await api.disconnect();
    return;
  }

  const isTestnet = await api.query.parameters.isTestnet();
  const useTestnetFlow = isTestnet.isTrue;

  if (useTestnetFlow) {
    console.log("\n  IsTestnet: true (using fast governance)\n");
    await setupViaGovernance(api, alice);
  } else {
    console.log("\n  IsTestnet: false (using TC proposal)\n");
    await setupViaTcProposal(api, keyring);
  }

  await sleep(6000);

  const finalWeth = await api.query.assetRegistry.locationAssets(wethLoc);
  const finalId = finalWeth.isSome ? finalWeth.toJSON() : null;
  const finalAccepted = finalId ? await api.query.multiTransactionPayment.acceptedCurrencies(finalId) : null;
  console.log("\n  Verification:");
  console.log("  WETH asset ID:  " + (finalId || "MISSING"));
  console.log("  WETH accepted:  " + (finalAccepted && finalAccepted.isSome ? "SET" : "MISSING"));

  if (finalId && finalAccepted && finalAccepted.isSome) {
    console.log("\n  Setup complete!\n");
  } else {
    console.log("\n  WARNING: Setup may still be pending. Wait a few blocks and re-run.\n");
  }

  await api.disconnect();
}

async function setupViaGovernance(api, alice) {
  const wethLocation = {
    parents: 1,
    interior: {
      X3: [
        { Parachain: 2004 },
        { PalletInstance: 110 },
        { AccountKey20: { key: "0xab3f0245b83feb11d15aaffefd7ad465a59817ed" } },
      ],
    },
  };

  const updateCall = api.tx.assetRegistry.update(20, null, null, null, null, null, null, null, wethLocation);
  const addCurrencyCall = api.tx.multiTransactionPayment.addCurrency(20, "1000000000000000000");
  const batchCall = api.tx.utility.batchAll([updateCall, addCurrencyCall]);

  const encodedCall = batchCall.method.toHex();
  const encodedHash = blake2AsHex(encodedCall);

  console.log("  [1/4] Noting preimage...");
  try {
    await sendAndWait(api.tx.preimage.notePreimage(encodedCall), alice, api);
  } catch (err) {
    if (!err.message.includes("AlreadyNoted")) throw err;
  }

  console.log("  [2/4] Submitting referendum (GeneralAdmin)...");
  const proposal = { Lookup: { hash: encodedHash, len: encodedCall.length / 2 - 1 } };
  const { events } = await sendAndWait(
    api.tx.referenda.submit({ Origins: "GeneralAdmin" }, proposal, { After: 1 }),
    alice, api
  );
  const submitted = events.find(({ event }) => event.section === "referenda" && event.method === "Submitted");
  const refIndex = submitted.event.data[0].toNumber();
  console.log(`    Referendum #${refIndex}`);

  console.log("  [3/4] Decision deposit + vote...");
  await sendAndWait(api.tx.referenda.placeDecisionDeposit(refIndex), alice, api);
  const { data: aliceData } = await api.query.system.account(alice.address);
  const voteAmount = aliceData.free.toBigInt() * 9n / 10n;
  await sendAndWait(
    api.tx.convictionVoting.vote(refIndex, {
      Standard: { balance: voteAmount, vote: { aye: true, conviction: "Locked1x" } },
    }),
    alice, api
  );

  console.log("  [4/4] Waiting for referendum...");
  for (let i = 0; i < 30; i++) {
    await sleep(6000);
    const info = await api.query.referenda.referendumInfoFor(refIndex);
    const json = info.toJSON();
    if (json.approved) { console.log(`    Referendum #${refIndex} approved!`); return; }
    if (json.rejected) throw new Error("Referendum rejected");
    if (json.timedOut) throw new Error("Referendum timed out");
    console.log(`    Waiting... block ${i + 1}`);
  }
  throw new Error("Timed out waiting for referendum");
}

async function executeTcProposal(api, alice, bob, call, label) {
  const threshold = 2;
  const proposalLen = call.method.encodedLength;

  console.log(`  Proposing: ${label}...`);
  const { events } = await sendAndWait(
    api.tx.technicalCommittee.propose(threshold, call, proposalLen),
    alice, api
  );
  const proposed = events.find(
    ({ event }) => event.section === "technicalCommittee" && event.method === "Proposed"
  );
  if (!proposed) throw new Error("Proposed event not found");
  const proposalIndex = proposed.event.data[1].toNumber();
  const proposalHash = proposed.event.data[2].toHex();
  console.log(`    Proposal #${proposalIndex}`);

  await sendAndWait(
    api.tx.technicalCommittee.vote(proposalHash, proposalIndex, true),
    alice, api
  );
  await sendAndWait(
    api.tx.technicalCommittee.vote(proposalHash, proposalIndex, true),
    bob, api
  );
  console.log("    Alice + Bob voted aye");

  const { events: closeEvents } = await sendAndWait(
    api.tx.technicalCommittee.close(
      proposalHash,
      proposalIndex,
      { refTime: 1000000000, proofSize: 100000 },
      proposalLen
    ),
    alice, api
  );

  const executed = closeEvents.find(
    ({ event }) => event.section === "technicalCommittee" && event.method === "Executed"
  );
  if (executed) {
    const result = executed.event.data.toJSON();
    console.log(`    Executed: ${JSON.stringify(result)}`);
  }
}

async function setupViaTcProposal(api, keyring) {
  const alice = keyring.addFromUri("//Alice");
  const bob = keyring.addFromUri("//Bob");

  const wethLocation = {
    parents: 1,
    interior: {
      X3: [
        { Parachain: 2004 },
        { PalletInstance: 110 },
        { AccountKey20: { key: "0xab3f0245b83feb11d15aaffefd7ad465a59817ed" } },
      ],
    },
  };

  // Step 1: Register WETH via register_external (any signed origin, auto-assigns ID)
  const wethAsset = await api.query.assetRegistry.assets(20);
  const existingLocation = await api.query.assetRegistry.assetLocations(20);

  if (!existingLocation.isSome) {
    // Check if an asset with this location already exists (from a previous register_external)
    const locationAssets = await api.query.assetRegistry.locationAssets(wethLocation);
    if (locationAssets.isSome) {
      const existingId = locationAssets.toJSON();
      console.log(`  WETH already registered as asset ${existingId} (via location lookup)`);
    } else {
      console.log("  Registering WETH via register_external...");
      try {
        const { events } = await sendAndWait(
          api.tx.assetRegistry.registerExternal(wethLocation),
          alice, api
        );
        const registered = events.find(
          ({ event }) => event.section === "assetRegistry" && event.method === "Registered"
        );
        if (registered) {
          const assetId = registered.event.data[0].toString();
          console.log(`    WETH registered as asset ${assetId}`);
        }
      } catch (err) {
        console.log(`    register_external error: ${err.message}`);
      }
    }
  } else {
    console.log("  WETH location already set for asset 20");
  }

  // Step 2: Resolve the WETH asset ID by location
  await sleep(2000);
  const resolvedAsset = await api.query.assetRegistry.locationAssets(wethLocation);
  if (!resolvedAsset.isSome) {
    console.log("  ERROR: Could not resolve WETH asset ID from location");
    return;
  }
  const wethId = resolvedAsset.toJSON();
  console.log(`  Resolved WETH asset ID: ${wethId}`);

  // Step 3: Add WETH as accepted fee currency via TC
  const wethAccepted = await api.query.multiTransactionPayment.acceptedCurrencies(wethId);
  if (!wethAccepted.isSome) {
    try {
      const addCurrencyCall = api.tx.multiTransactionPayment.addCurrency(wethId, "1000000000000000000");
      await executeTcProposal(api, alice, bob, addCurrencyCall, "addCurrency for WETH");
    } catch (err) {
      console.log(`    Error: ${err.message}`);
    }
  } else {
    console.log("  WETH already accepted as fee currency");
  }
}

main().catch((err) => {
  console.error("\n  Setup FAILED:", err.message);
  process.exit(1);
});
