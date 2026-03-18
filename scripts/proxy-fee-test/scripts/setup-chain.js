const { ApiPromise, WsProvider } = require("@polkadot/api");
const { Keyring } = require("@polkadot/keyring");
const { blake2AsHex } = require("@polkadot/util-crypto");

const WS_URL = process.env.WS_URL || "ws://127.0.0.1:9999";

function sendAndWait(tx, signer, api) {
  return new Promise((resolve, reject) => {
    tx.signAndSend(signer, ({ status, events, dispatchError }) => {
      if (dispatchError) {
        if (dispatchError.isModule) {
          const decoded = api.registry.findMetaError(dispatchError.asModule);
          reject(new Error(`${decoded.section}.${decoded.name}: ${decoded.docs.join(" ")}`));
        } else {
          reject(new Error(dispatchError.toString()));
        }
        return;
      }
      if (status.isInBlock) {
        resolve({ blockHash: status.asInBlock, events });
      }
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

  const wethLocation = await api.query.assetRegistry.assetLocations(20);
  const wethAccepted = await api.query.multiTransactionPayment.acceptedCurrencies(20);

  if (wethLocation.isSome && wethAccepted.isSome) {
    console.log("  WETH location:  already set");
    console.log("  WETH accepted:  already set");
    console.log("\n  Chain is already configured. Skipping setup.\n");
    await api.disconnect();
    return;
  }

  console.log("  WETH location:  " + (wethLocation.isSome ? "set" : "NOT SET"));
  console.log("  WETH accepted:  " + (wethAccepted.isSome ? "set" : "NOT SET"));

  const isTestnet = await api.query.parameters.isTestnet();
  const useTestnetFlow = isTestnet.isTrue;

  if (useTestnetFlow) {
    console.log("  IsTestnet: true (using fast governance)\n");
    await setupViaGovernance(api, alice);
  } else {
    console.log("  IsTestnet: false (using TC proposal)\n");
    await setupViaTcProposal(api, keyring);
  }

  await sleep(6000);

  const newLocation = await api.query.assetRegistry.assetLocations(20);
  const newAccepted = await api.query.multiTransactionPayment.acceptedCurrencies(20);
  console.log("\n  Verification:");
  console.log("  WETH location: " + (newLocation.isSome ? "SET" : "MISSING"));
  console.log("  WETH accepted: " + (newAccepted.isSome ? "SET" : "MISSING"));

  if (newLocation.isSome && newAccepted.isSome) {
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

  const updateCall = api.tx.assetRegistry.update(20, null, null, null, null, null, null, null, wethLocation);
  const addCurrencyCall = api.tx.multiTransactionPayment.addCurrency(20, "1000000000000000000");
  const batchCall = api.tx.utility.batchAll([updateCall, addCurrencyCall]);

  const tcIndex = api.consts.technicalCommittee
    ? undefined
    : 2;

  console.log("  [1/3] Submitting TC proposal...");
  const encodedProposal = batchCall.method.toHex();
  const proposalHash = blake2AsHex(encodedProposal);
  const proposalLen = batchCall.method.encodedLength;

  try {
    await sendAndWait(
      api.tx.preimage.notePreimage(encodedProposal),
      alice, api
    );
    console.log("    Preimage noted");
  } catch (err) {
    if (!err.message.includes("AlreadyNoted")) throw err;
    console.log("    Preimage already noted");
  }

  let proposalIndex;
  try {
    const { events } = await sendAndWait(
      api.tx.technicalCommittee.propose(
        2,
        batchCall,
        proposalLen
      ),
      alice, api
    );
    const proposed = events.find(
      ({ event }) => event.section === "technicalCommittee" && event.method === "Proposed"
    );
    if (proposed) {
      proposalIndex = proposed.event.data[1].toNumber();
      console.log(`    TC proposal #${proposalIndex}`);
    }
  } catch (err) {
    console.log(`    TC propose error: ${err.message}`);
    console.log("    Trying direct GeneralAdmin referendum instead...");
    await setupViaGovernance(api, alice);
    return;
  }

  console.log("  [2/3] Alice + Bob voting aye...");
  const proposalHashFromChain = proposalHash;

  try {
    await sendAndWait(
      api.tx.technicalCommittee.vote(proposalHashFromChain, proposalIndex, true),
      alice, api
    );
    console.log("    Alice voted aye");
  } catch (err) {
    console.log(`    Alice vote: ${err.message}`);
  }

  try {
    await sendAndWait(
      api.tx.technicalCommittee.vote(proposalHashFromChain, proposalIndex, true),
      bob, api
    );
    console.log("    Bob voted aye");
  } catch (err) {
    console.log(`    Bob vote: ${err.message}`);
  }

  console.log("  [3/3] Closing proposal...");
  try {
    await sendAndWait(
      api.tx.technicalCommittee.close(
        proposalHashFromChain,
        proposalIndex,
        { refTime: 1000000000, proofSize: 100000 },
        proposalLen
      ),
      alice, api
    );
    console.log("    Proposal closed and executed");
  } catch (err) {
    console.log(`    Close: ${err.message}`);
  }
}

main().catch((err) => {
  console.error("\n  Setup FAILED:", err.message);
  process.exit(1);
});
