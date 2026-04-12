/**
 * setup-chain.js
 *
 * Configures the running chain for EVM gas estimation testing:
 *   1. Sets the WETH XCM location (so WethAssetId resolves to asset 20)
 *   2. Adds WETH as an accepted fee currency (so EVM fee withdrawal works)
 *
 * Both calls are batched into a single GeneralAdmin referendum.
 * Requires IsTestnet=true in genesis (set by build-chainspec.sh)
 * so governance tracks have 1-block periods.
 *
 * Prerequisites:
 *   - Run ./scripts/build-chainspec.sh first (generates chainspec-raw.json)
 *   - Zombienet running with zombienet.json
 *
 * Usage: node scripts/setup-chain.js
 * Env:   WS_URL (default: ws://127.0.0.1:9999)
 */

const { ApiPromise, WsProvider } = require("@polkadot/api");
const { Keyring } = require("@polkadot/keyring");
const { blake2AsHex } = require("@polkadot/util-crypto");

const WS_URL = process.env.WS_URL || "ws://127.0.0.1:9999";

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

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

async function main() {
  console.log("=".repeat(60));
  console.log("  Chain Setup: WETH Location + Accepted Fee Currency");
  console.log("=".repeat(60));
  console.log(`\n  RPC: ${WS_URL}`);

  const provider = new WsProvider(WS_URL);
  const api = await ApiPromise.create({ provider });

  const [chain, nodeName] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.name(),
  ]);
  console.log(`  Connected to: ${chain} (${nodeName})\n`);

  // Verify testnet mode is active
  const isTestnet = await api.query.parameters.isTestnet();
  if (!isTestnet.isTrue) {
    throw new Error(
      "Parameters::IsTestnet is false. Run ./scripts/build-chainspec.sh and restart zombienet."
    );
  }
  console.log("  IsTestnet: true (fast governance tracks active)");

  const keyring = new Keyring({ type: "sr25519" });
  const alice = keyring.addFromUri("//Alice");

  // Check Alice's balance
  const { data: aliceData } = await api.query.system.account(alice.address);
  const aliceFree = aliceData.free.toBigInt();
  console.log(`  Alice balance: ${aliceFree} (${Number(aliceFree / 1000000000000n)} UNITS)`);

  // Check if setup was already done (WETH location set = EVM balance visible)
  const evmBalance = await api.rpc.eth
    .getBalance("0xC19A2970A13ac19898c47d59Cbd0278D428EBC7c")
    .catch(() => null);
  if (evmBalance && BigInt(evmBalance.toHex()) > 0n) {
    console.log("  WETH location already set. EVM balance:", evmBalance.toHex());
    console.log("  Skipping setup.\n");
    await api.disconnect();
    return;
  }

  // Build a batch of two calls:
  // 1. Set WETH XCM location (needed for WethCurrency to resolve asset ID)
  // 2. Add WETH as accepted fee currency (needed for EVM fee withdrawal)
  const wethLocation = {
    parents: 1,
    interior: {
      X3: [
        { Parachain: 2004 },
        { PalletInstance: 110 },
        {
          AccountKey20: {
            key: "0xab3f0245b83feb11d15aaffefd7ad465a59817ed",
          },
        },
      ],
    },
  };

  const updateCall = api.tx.assetRegistry.update(
    20, null, null, null, null, null, null, null, wethLocation
  );
  // Price::from(1) = FixedU128(1e18) - price of WETH in native asset units
  const addCurrencyCall = api.tx.multiTransactionPayment.addCurrency(
    20, "1000000000000000000"
  );
  const batchCall = api.tx.utility.batchAll([updateCall, addCurrencyCall]);

  const encodedCall = batchCall.method.toHex();
  const encodedHash = blake2AsHex(encodedCall);

  // Step 1: Note preimage
  console.log("\n  [1/5] Noting preimage...");
  try {
    await sendAndWait(api.tx.preimage.notePreimage(encodedCall), alice, api);
    console.log("    Preimage noted.");
  } catch (err) {
    if (err.message.includes("AlreadyNoted")) {
      console.log("    Already noted, skipping.");
    } else {
      throw err;
    }
  }

  // Step 2: Submit referendum
  console.log("  [2/5] Submitting referendum (GeneralAdmin track)...");
  const proposal = {
    Lookup: { hash: encodedHash, len: encodedCall.length / 2 - 1 },
  };
  const { events: submitEvents } = await sendAndWait(
    api.tx.referenda.submit({ Origins: "GeneralAdmin" }, proposal, { After: 1 }),
    alice,
    api
  );
  const submittedEvent = submitEvents.find(
    ({ event }) => event.section === "referenda" && event.method === "Submitted"
  );
  if (!submittedEvent) throw new Error("No Submitted event found");
  const refIndex = submittedEvent.event.data[0].toNumber();
  console.log(`    Referendum #${refIndex} submitted.`);

  // Step 3: Place decision deposit
  console.log("  [3/5] Placing decision deposit...");
  await sendAndWait(api.tx.referenda.placeDecisionDeposit(refIndex), alice, api);
  console.log("    Decision deposit placed.");

  // Step 4: Vote AYE
  console.log("  [4/5] Alice voting AYE...");
  const voteAmount = aliceFree * 9n / 10n;
  await sendAndWait(
    api.tx.convictionVoting.vote(refIndex, {
      Standard: { balance: voteAmount, vote: { aye: true, conviction: "Locked1x" } },
    }),
    alice,
    api
  );
  console.log(`    Voted AYE with ${voteAmount} tokens.`);

  // Step 5: Wait for approval
  console.log("  [5/5] Waiting for referendum to pass...");
  for (let i = 0; i < 60; i++) {
    await sleep(6000);

    const info = await api.query.referenda.referendumInfoFor(refIndex);
    const infoJson = info.toJSON();

    if (infoJson.approved) {
      console.log(`    Referendum #${refIndex} approved and enacted!`);
      break;
    } else if (infoJson.rejected) {
      throw new Error(`Referendum #${refIndex} was rejected`);
    } else if (infoJson.timedOut) {
      throw new Error(`Referendum #${refIndex} timed out`);
    }

    const ongoing = infoJson.ongoing;
    if (ongoing && ongoing.deciding) {
      console.log(`    Block ${i + 1}: deciding, confirming=${!!ongoing.deciding.confirming}`);
    } else {
      console.log(`    Block ${i + 1}: preparing`);
    }
  }

  // Verify
  console.log("\n  Verifying EVM setup...");
  await sleep(6000);

  const newBalance = await api.rpc.eth
    .getBalance("0xC19A2970A13ac19898c47d59Cbd0278D428EBC7c")
    .catch(() => "0x0");
  const balWei = BigInt(newBalance.toString());
  console.log(`  EVM balance: ${balWei / 10n ** 18n} WETH (${balWei} wei)`);

  if (balWei > 0n) {
    console.log("\n  Setup complete! EVM is ready.");
  } else {
    console.log("\n  WARNING: EVM balance still 0.");
  }

  console.log("  You can now run: npm run test:gas\n");
  await api.disconnect();
}

main().catch((err) => {
  console.error("\n  Setup FAILED:", err.message);
  console.error("  Make sure zombienet is running with the correct config.\n");
  process.exit(1);
});
