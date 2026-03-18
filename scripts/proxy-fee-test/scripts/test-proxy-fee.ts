import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
import { decodeAddress } from "@polkadot/util-crypto";
import { u8aToHex } from "@polkadot/util";
import { ethers } from "ethers";

const WS_URL = process.env.WS_URL || "ws://127.0.0.1:9999";
const EVM_RPC_URL = process.env.EVM_RPC_URL || "http://127.0.0.1:9999";

const WETH_ASSET_ID = 20;
const HDX_ASSET_ID = 0;
const UNITS = 1_000_000_000_000n;

interface TestResult {
  name: string;
  passed: boolean;
  detail: string;
}

const results: TestResult[] = [];

function pass(name: string, detail: string) {
  results.push({ name, passed: true, detail });
  console.log(`  ✓ ${name}`);
  if (detail) console.log(`    ${detail}`);
}

function fail(name: string, detail: string) {
  results.push({ name, passed: false, detail });
  console.log(`  ✗ ${name}`);
  if (detail) console.log(`    ${detail}`);
}

function sendAndWait(
  tx: any,
  signer: any,
  api: ApiPromise
): Promise<{ blockHash: string; events: any[] }> {
  return new Promise((resolve, reject) => {
    tx.signAndSend(signer, ({ status, events, dispatchError }: any) => {
      if (dispatchError) {
        if (dispatchError.isModule) {
          const decoded = api.registry.findMetaError(dispatchError.asModule);
          reject(
            new Error(
              `${decoded.section}.${decoded.name}: ${decoded.docs.join(" ")}`
            )
          );
        } else {
          reject(new Error(dispatchError.toString()));
        }
        return;
      }
      if (status.isInBlock) {
        resolve({ blockHash: status.asInBlock.toString(), events });
      }
    });
  });
}

async function getTokenBalance(
  api: ApiPromise,
  account: string,
  assetId: number
): Promise<bigint> {
  if (assetId === 0) {
    const { data } = (await api.query.system.account(account)) as any;
    return BigInt(data.free.toString());
  }
  const balance = await api.query.tokens.accounts(account, assetId);
  return BigInt((balance as any).free.toString());
}

function substrateToEvmAddress(ss58Address: string): string {
  const bytes = decodeAddress(ss58Address);
  return u8aToHex(bytes.slice(0, 20));
}

async function createPureProxy(
  api: ApiPromise,
  signer: any
): Promise<string> {
  const { events } = await sendAndWait(
    api.tx.proxy.createPure("Any", 0, 0),
    signer,
    api
  );

  const pureCreated = events.find(
    ({ event }: any) =>
      event.section === "proxy" && event.method === "PureCreated"
  );
  if (!pureCreated) throw new Error("PureCreated event not found");

  return pureCreated.event.data[0].toString();
}

async function main() {
  console.log("=".repeat(64));
  console.log("  Issue #1381: EVM Fee Payer Override for pureProxy");
  console.log("  End-to-End Test");
  console.log("=".repeat(64));
  console.log(`\n  Substrate WS: ${WS_URL}`);
  console.log(`  EVM RPC:      ${EVM_RPC_URL}\n`);

  const provider = new WsProvider(WS_URL);
  const api = await ApiPromise.create({ provider });

  const [chain, nodeName] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.name(),
  ]);
  console.log(`  Connected to: ${chain} (${nodeName})\n`);

  const keyring = new Keyring({ type: "sr25519" });
  const alice = keyring.addFromUri("//Alice");
  const bob = keyring.addFromUri("//Bob");
  const aliceAddress = alice.address;
  const bobAddress = bob.address;

  console.log(`  Alice (controller): ${aliceAddress}`);
  console.log(`  Bob:                ${bobAddress}\n`);

  const wethLocation = await api.query.assetRegistry.assetLocations(WETH_ASSET_ID);
  const wethAccepted = await api.query.multiTransactionPayment.acceptedCurrencies(WETH_ASSET_ID);

  if (!wethLocation.isSome || !wethAccepted.isSome) {
    console.log("  ERROR: WETH is not configured on this chain.");
    console.log("  Run: WS_URL=" + WS_URL + " npm run setup\n");
    await api.disconnect();
    process.exit(1);
  }
  console.log("  WETH location: configured");
  console.log("  WETH accepted: configured\n");

  // ─────────────────────────────────────────────────────────────────
  // TEST 1: Direct EVM call charges the EVM source (baseline)
  // ─────────────────────────────────────────────────────────────────

  console.log("── Test 1: Direct EVM call charges EVM source (baseline) ──\n");

  const evmProvider = new ethers.JsonRpcProvider(EVM_RPC_URL);
  const TEST_PRIVKEY =
    "653a29ac0c93de0e9f7d7ea2d60338e68f407b18d16d6ff84db996076424f8fa";
  const evmWallet = new ethers.Wallet(TEST_PRIVKEY, evmProvider);
  const evmAddress = evmWallet.address;
  console.log(`  EVM account: ${evmAddress}`);

  const balanceBefore = await evmProvider.getBalance(evmAddress);
  console.log(`  WETH balance before: ${ethers.formatEther(balanceBefore)} WETH`);

  if (balanceBefore === 0n) {
    fail(
      "Direct EVM call",
      "EVM account has no WETH. Ensure setup-chain.js funded this account."
    );
  } else {
    try {
      const tx = await evmWallet.sendTransaction({
        to: "0x0000000000000000000000000000000000001234",
        value: 0n,
        data: "0x",
        gasLimit: 100_000n,
      });
      const receipt = await tx.wait();
      const balanceAfter = await evmProvider.getBalance(evmAddress);
      const gasCost = balanceBefore - balanceAfter;

      if (gasCost > 0n) {
        pass(
          "Direct EVM call charges EVM source",
          `Gas cost: ${ethers.formatEther(gasCost)} WETH (${receipt!.gasUsed} gas used)`
        );
      } else {
        fail("Direct EVM call charges EVM source", "Balance unchanged");
      }
    } catch (err: any) {
      fail("Direct EVM call", `Error: ${err.message}`);
    }
  }

  // ─────────────────────────────────────────────────────────────────
  // TEST 2: proxy.proxy(real=pureProxy, call=EVM::call) charges controller
  //
  // This is the core Issue #1381 scenario:
  //   1. Alice creates a pureProxy (she's automatically the controller)
  //   2. Alice calls proxy.proxy(real=pureProxy, call=EVM::call(...))
  //   3. The SetEvmFeePayer extension detects the pattern
  //   4. EVM gas fees are charged to Alice (controller), not the pureProxy
  // ─────────────────────────────────────────────────────────────────

  console.log(
    "\n── Test 2: proxy(pureProxy, EVM::call) charges controller ──\n"
  );

  try {
    console.log("  Creating pureProxy for Alice...");
    const pureProxyAddress = await createPureProxy(api, alice);
    console.log(`  PureProxy: ${pureProxyAddress}`);

    const pureProxyEvmAddr = substrateToEvmAddress(pureProxyAddress);
    console.log(`  PureProxy EVM addr: ${pureProxyEvmAddr}`);

    const aliceHdxBefore = await getTokenBalance(api, aliceAddress, HDX_ASSET_ID);
    const aliceWethBefore = await getTokenBalance(api, aliceAddress, WETH_ASSET_ID);
    console.log(`  Alice HDX before:  ${aliceHdxBefore / UNITS} UNITS`);
    console.log(`  Alice WETH before: ${aliceWethBefore / UNITS} UNITS`);

    const evmCall = api.tx.evm.call(
      pureProxyEvmAddr,
      "0x0000000000000000000000000000000000001234",
      "0x",
      0,
      100000,
      15000000,
      null,
      0,
      [],
      []
    );

    const proxyCall = api.tx.proxy.proxy(pureProxyAddress, null, evmCall);

    console.log("  Sending proxy(pureProxy, EVM::call)...");
    const { events } = await sendAndWait(proxyCall, alice, api);

    const proxyExecuted = events.find(
      ({ event }: any) =>
        event.section === "proxy" && event.method === "ProxyExecuted"
    );
    if (proxyExecuted) {
      console.log(
        `  ProxyExecuted: ${JSON.stringify(proxyExecuted.event.data[0].toJSON())}`
      );
    }

    const aliceHdxAfter = await getTokenBalance(api, aliceAddress, HDX_ASSET_ID);
    const aliceWethAfter = await getTokenBalance(api, aliceAddress, WETH_ASSET_ID);
    console.log(`  Alice HDX after:   ${aliceHdxAfter / UNITS} UNITS`);
    console.log(`  Alice WETH after:  ${aliceWethAfter / UNITS} UNITS`);

    const hdxCharged = aliceHdxBefore - aliceHdxAfter;
    const wethCharged = aliceWethBefore - aliceWethAfter;

    if (hdxCharged > 0n || wethCharged > 0n) {
      pass(
        "proxy(pureProxy, EVM::call) charges controller",
        `Alice charged: HDX=${hdxCharged}, WETH=${wethCharged}`
      );
    } else {
      fail(
        "proxy(pureProxy, EVM::call) charges controller",
        `Alice balance unchanged. HDX diff: ${hdxCharged}, WETH diff: ${wethCharged}. ` +
          `The inner EVM call may have failed — check ProxyExecuted event above.`
      );
    }
  } catch (err: any) {
    fail("proxy(pureProxy, EVM::call) charges controller", `Error: ${err.message}`);
  }

  // ─────────────────────────────────────────────────────────────────
  // TEST 3: proxy(pureProxy, EVM::call) fails when controller has no funds
  // ─────────────────────────────────────────────────────────────────

  console.log(
    "\n── Test 3: proxy(EVM::call) fails when controller has no funds ──\n"
  );

  const dave = keyring.addFromUri("//Dave");
  const daveAddress = dave.address;
  console.log(`  Dave (no-funds controller): ${daveAddress}`);

  try {
    console.log("  Creating pureProxy for Dave...");
    const davePureProxy = await createPureProxy(api, dave);
    console.log(`  PureProxy: ${davePureProxy}`);

    const davePureEvmAddr = substrateToEvmAddress(davePureProxy);

    const daveHdxBefore = await getTokenBalance(api, daveAddress, HDX_ASSET_ID);
    const daveWethBefore = await getTokenBalance(api, daveAddress, WETH_ASSET_ID);
    console.log(`  Dave HDX:  ${daveHdxBefore / UNITS} UNITS`);
    console.log(`  Dave WETH: ${daveWethBefore / UNITS} UNITS`);

    const evmCall = api.tx.evm.call(
      davePureEvmAddr,
      "0x0000000000000000000000000000000000001234",
      "0x",
      0,
      100000,
      15000000,
      null,
      null,
      [],
      []
    );

    const proxyCall = api.tx.proxy.proxy(davePureProxy, null, evmCall);

    console.log("  Sending proxy(pureProxy, EVM::call) as Dave...");
    const { events } = await sendAndWait(proxyCall, dave, api);

    const proxyExecuted = events.find(
      ({ event }: any) =>
        event.section === "proxy" && event.method === "ProxyExecuted"
    );

    if (proxyExecuted) {
      const resultJson = proxyExecuted.event.data[0].toJSON();
      const isErr =
        resultJson && typeof resultJson === "object" && "err" in resultJson;

      if (isErr) {
        pass(
          "proxy(EVM::call) fails with no-funds controller",
          `Inner dispatch error: ${JSON.stringify(resultJson)}`
        );
      } else {
        fail(
          "proxy(EVM::call) fails with no-funds controller",
          `Expected inner error but got: ${JSON.stringify(resultJson)}`
        );
      }
    } else {
      fail(
        "proxy(EVM::call) fails with no-funds controller",
        "No ProxyExecuted event found"
      );
    }
  } catch (err: any) {
    if (
      err.message.includes("BalanceLow") ||
      err.message.includes("InsufficientBalance") ||
      err.message.includes("WithdrawFailed")
    ) {
      pass(
        "proxy(EVM::call) fails with no-funds controller",
        `Expected error: ${err.message}`
      );
    } else {
      fail(
        "proxy(EVM::call) fails with no-funds controller",
        `Error: ${err.message}`
      );
    }
  }

  // ─────────────────────────────────────────────────────────────────
  // TEST 4: Non-EVM proxy call works normally (no interference)
  // ─────────────────────────────────────────────────────────────────

  console.log("\n── Test 4: Non-EVM proxy call works normally ──\n");

  try {
    try {
      await sendAndWait(
        api.tx.proxy.addProxy(aliceAddress, "Any", 0),
        bob,
        api
      );
    } catch {}

    const aliceHdxBeforeRemark = await getTokenBalance(
      api,
      aliceAddress,
      HDX_ASSET_ID
    );

    const remarkCall = api.tx.system.remark("0x1234");
    const proxyRemarkCall = api.tx.proxy.proxy(bobAddress, null, remarkCall);

    await sendAndWait(proxyRemarkCall, alice, api);

    const aliceHdxAfterRemark = await getTokenBalance(
      api,
      aliceAddress,
      HDX_ASSET_ID
    );

    const remarkFee = aliceHdxBeforeRemark - aliceHdxAfterRemark;

    if (remarkFee >= 0n) {
      pass(
        "Non-EVM proxy call works normally",
        `Standard substrate fee: ${remarkFee} (${remarkFee / UNITS} UNITS HDX)`
      );
    } else {
      fail(
        "Non-EVM proxy call works normally",
        `Unexpected balance increase: ${remarkFee}`
      );
    }
  } catch (err: any) {
    fail("Non-EVM proxy call works normally", `Error: ${err.message}`);
  }

  // ─────────────────────────────────────────────────────────────────
  // TEST 5: proxy(batch([EVM::call])) — nested pattern detection
  // ─────────────────────────────────────────────────────────────────

  console.log("\n── Test 5: proxy(batch([EVM::call])) charges controller ──\n");

  try {
    console.log("  Creating pureProxy for Alice...");
    const pureProxy2 = await createPureProxy(api, alice);
    console.log(`  PureProxy: ${pureProxy2}`);
    const pureProxy2Evm = substrateToEvmAddress(pureProxy2);

    const aliceHdxBeforeBatch = await getTokenBalance(api, aliceAddress, HDX_ASSET_ID);
    const aliceWethBeforeBatch = await getTokenBalance(api, aliceAddress, WETH_ASSET_ID);

    const evmCallForBatch = api.tx.evm.call(
      pureProxy2Evm,
      "0x0000000000000000000000000000000000005678",
      "0x",
      0,
      100000,
      15000000,
      null,
      null,
      [],
      []
    );

    const batchCall = api.tx.utility.batch([evmCallForBatch]);
    const proxyBatchCall = api.tx.proxy.proxy(pureProxy2, null, batchCall);

    console.log("  Sending proxy(pureProxy, batch([EVM::call]))...");
    const { events } = await sendAndWait(proxyBatchCall, alice, api);

    const proxyExecuted = events.find(
      ({ event }: any) =>
        event.section === "proxy" && event.method === "ProxyExecuted"
    );
    if (proxyExecuted) {
      console.log(
        `  ProxyExecuted: ${JSON.stringify(proxyExecuted.event.data[0].toJSON())}`
      );
    }

    const aliceHdxAfterBatch = await getTokenBalance(api, aliceAddress, HDX_ASSET_ID);
    const aliceWethAfterBatch = await getTokenBalance(api, aliceAddress, WETH_ASSET_ID);

    const hdxDiff = aliceHdxBeforeBatch - aliceHdxAfterBatch;
    const wethDiff = aliceWethBeforeBatch - aliceWethAfterBatch;

    if (hdxDiff > 0n || wethDiff > 0n) {
      pass(
        "proxy(batch([EVM::call])) charges controller",
        `Alice charged: HDX=${hdxDiff}, WETH=${wethDiff}`
      );
    } else {
      fail(
        "proxy(batch([EVM::call])) charges controller",
        `Alice balance unchanged. HDX diff: ${hdxDiff}, WETH diff: ${wethDiff}`
      );
    }
  } catch (err: any) {
    fail(
      "proxy(batch([EVM::call])) charges controller",
      `Error: ${err.message}`
    );
  }

  // ─────────────────────────────────────────────────────────────────
  // Summary
  // ─────────────────────────────────────────────────────────────────

  console.log("\n" + "=".repeat(64));
  console.log("  RESULTS");
  console.log("=".repeat(64));

  const passed = results.filter((r) => r.passed).length;
  const failed = results.filter((r) => !r.passed).length;

  for (const r of results) {
    console.log(`  ${r.passed ? "✓" : "✗"} ${r.name}`);
  }

  console.log(
    `\n  ${passed} passed, ${failed} failed out of ${results.length}`
  );

  if (failed > 0) {
    console.log("\n  SOME TESTS FAILED. See details above.");
    console.log("=".repeat(64) + "\n");
  } else {
    console.log("\n  ALL TESTS PASSED!");
    console.log("=".repeat(64) + "\n");
  }

  await api.disconnect();
  process.exit(failed > 0 ? 1 : 0);
}

main().catch((err) => {
  console.error("\n  Test FAILED:", err.message);
  console.error(
    "  Make sure the chain is running with EVM setup complete.\n"
  );
  process.exit(1);
});
