const { ApiPromise, WsProvider } = require("@polkadot/api");
const { Keyring } = require("@polkadot/keyring");
const { blake2AsHex } = require("@polkadot/util-crypto");
const fs = require("fs");
const path = require("path");

const WS_URL = process.env.WS_URL || "wss://2.lark.hydration.cloud";
const WASM_PATH =
  process.env.WASM_PATH ||
  path.resolve(
    __dirname,
    "../../../target/release/wbuild/hydradx-runtime/hydradx_runtime.compact.compressed.wasm"
  );

function sendAndWait(tx, signer, api, timeoutMs = 120000) {
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
        resolve({ blockHash: status.asInBlock.toString(), events });
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
  console.log("  Deploy Runtime to Lark 2");
  console.log("=".repeat(60));
  console.log(`\n  RPC:  ${WS_URL}`);
  console.log(`  WASM: ${WASM_PATH}\n`);

  if (!fs.existsSync(WASM_PATH)) {
    console.error(`  ERROR: WASM file not found at ${WASM_PATH}`);
    console.error("  Run: make build-release");
    process.exit(1);
  }

  const wasmBytes = fs.readFileSync(WASM_PATH);
  const wasmHex = "0x" + wasmBytes.toString("hex");
  const codeHash = blake2AsHex(wasmBytes, 256);
  console.log(`  WASM size: ${(wasmBytes.length / 1024 / 1024).toFixed(2)} MB`);
  console.log(`  Code hash: ${codeHash}`);

  const provider = new WsProvider(WS_URL);
  const api = await ApiPromise.create({ provider });

  const [chain, nodeName] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.name(),
  ]);
  const runtimeVersion = await api.rpc.state.getRuntimeVersion();
  console.log(`  Chain:   ${chain} (${nodeName})`);
  console.log(`  Current: spec_version=${runtimeVersion.specVersion}\n`);

  const keyring = new Keyring({ type: "sr25519" });
  const alice = keyring.addFromUri("//Alice");
  console.log(`  Deployer: ${alice.address} (//Alice)\n`);

  const hasSudo = api.tx.sudo && Object.keys(api.tx.sudo).length > 0;

  if (hasSudo) {
    console.log("  Strategy: sudo\n");
    await deployViaSudo(api, alice, codeHash, wasmHex);
  } else {
    console.log("  Strategy: governance (referendum)\n");
    await deployViaGovernance(api, alice, codeHash, wasmHex);
  }

  await api.disconnect();
}

async function deployViaSudo(api, alice, codeHash, wasmHex) {
  console.log("  [1/2] Authorizing upgrade via sudo...");
  const authorizeCall = api.tx.system.authorizeUpgrade(codeHash);
  await sendAndWait(
    api.tx.sudo.sudoUncheckedWeight(authorizeCall, { refTime: 0, proofSize: 0 }),
    alice,
    api
  );
  console.log("    Upgrade authorized.");

  console.log("  [2/2] Applying upgrade (uploading WASM)...");
  console.log("    This may take a moment...");
  await sendAndWait(
    api.tx.system.applyAuthorizedUpgrade(wasmHex),
    alice,
    api
  );
  console.log("    WASM uploaded and applied!");

  await sleep(12000);
  const newVersion = await api.rpc.state.getRuntimeVersion();
  console.log(`\n  New spec_version: ${newVersion.specVersion}`);
  console.log("  Upgrade complete!\n");
}

async function deployViaGovernance(api, alice, codeHash, wasmHex) {
  const authorizeCall = api.tx.system.authorizeUpgrade(codeHash);
  const encodedCall = authorizeCall.method.toHex();
  const encodedHash = blake2AsHex(encodedCall);

  console.log("  [1/5] Noting preimage...");
  try {
    await sendAndWait(api.tx.preimage.notePreimage(encodedCall), alice, api);
    console.log("    Preimage noted.");
  } catch (err) {
    if (err.message.includes("AlreadyNoted")) {
      console.log("    Already noted.");
    } else {
      throw err;
    }
  }

  console.log("  [2/5] Submitting referendum (Root track)...");
  const proposal = {
    Lookup: { hash: encodedHash, len: encodedCall.length / 2 - 1 },
  };
  const { events: submitEvents } = await sendAndWait(
    api.tx.referenda.submit({ system: "Root" }, proposal, { After: 1 }),
    alice,
    api
  );
  const submitted = submitEvents.find(
    ({ event }) => event.section === "referenda" && event.method === "Submitted"
  );
  if (!submitted) throw new Error("No Submitted event");
  const refIndex = submitted.event.data[0].toNumber();
  console.log(`    Referendum #${refIndex}`);

  console.log("  [3/5] Placing decision deposit...");
  await sendAndWait(
    api.tx.referenda.placeDecisionDeposit(refIndex),
    alice,
    api
  );

  console.log("  [4/5] Voting AYE...");
  const { data: aliceData } = await api.query.system.account(alice.address);
  const voteAmount = aliceData.free.toBigInt() * 9n / 10n;
  await sendAndWait(
    api.tx.convictionVoting.vote(refIndex, {
      Standard: {
        balance: voteAmount,
        vote: { aye: true, conviction: "Locked1x" },
      },
    }),
    alice,
    api
  );
  console.log(`    Voted AYE with ${voteAmount / 1000000000000n} HDX`);

  console.log("  [5/5] Waiting for referendum to pass...");
  for (let i = 0; i < 120; i++) {
    await sleep(12000);
    const info = await api.query.referenda.referendumInfoFor(refIndex);
    const json = info.toJSON();
    if (json.approved) {
      console.log(`    Referendum #${refIndex} approved!`);
      break;
    }
    if (json.rejected) throw new Error("Referendum rejected");
    if (json.timedOut) throw new Error("Referendum timed out");
    const ongoing = json.ongoing;
    if (ongoing && ongoing.deciding) {
      console.log(`    Waiting... (${i + 1}) deciding, confirming=${!!ongoing.deciding.confirming}`);
    } else {
      console.log(`    Waiting... (${i + 1}) preparing`);
    }
  }

  console.log("\n  Applying upgrade (uploading WASM)...");
  console.log("  This may take a moment...");
  await sendAndWait(
    api.tx.system.applyAuthorizedUpgrade(wasmHex),
    alice,
    api
  );

  await sleep(12000);
  const newVersion = await api.rpc.state.getRuntimeVersion();
  console.log(`\n  New spec_version: ${newVersion.specVersion}`);
  console.log("  Upgrade complete!\n");
}

main().catch((err) => {
  console.error(`\n  Deploy FAILED: ${err.message}\n`);
  process.exit(1);
});
