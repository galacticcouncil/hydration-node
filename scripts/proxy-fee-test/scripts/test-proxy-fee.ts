import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
import { decodeAddress } from "@polkadot/util-crypto";
import { u8aToHex } from "@polkadot/util";
import { ethers } from "ethers";

const WS_URL = process.env.WS_URL || "ws://127.0.0.1:9999";
const EVM_RPC_URL = process.env.EVM_RPC_URL || "http://127.0.0.1:9999";

const HDX_ASSET_ID = 0;
const UNITS = 1_000_000_000_000n;

const WETH_LOCATION = {
  parents: 1,
  interior: {
    X3: [
      { Parachain: 2004 },
      { PalletInstance: 110 },
      { AccountKey20: { key: "0xab3f0245b83feb11d15aaffefd7ad465a59817ed" } },
    ],
  },
};

let WETH_ASSET_ID: number;

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
    }).catch((err: any) => reject(err));
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

function makeEvmCall(api: ApiPromise, sourceEvmAddr: string) {
  return api.tx.evm.call(
    sourceEvmAddr,
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
}

async function main() {
  console.log("=".repeat(64));
  console.log("  Issue #1381: EVM Fee Payer Override — dispatch_with_fee_payer");
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

  const wethByLocation = await (api.query.assetRegistry as any).locationAssets(WETH_LOCATION);
  if (!wethByLocation.isSome) {
    console.log("  ERROR: WETH is not registered on this chain.");
    console.log("  Run: WS_URL=" + WS_URL + " npm run setup\n");
    await api.disconnect();
    process.exit(1);
  }
  WETH_ASSET_ID = (wethByLocation as any).toJSON() as number;
  const wethAccepted = await api.query.multiTransactionPayment.acceptedCurrencies(WETH_ASSET_ID);

  if (!wethAccepted.isSome) {
    console.log("  ERROR: WETH not accepted as fee currency.");
    console.log("  Run: WS_URL=" + WS_URL + " npm run setup\n");
    await api.disconnect();
    process.exit(1);
  }
  console.log(`  WETH asset ID: ${WETH_ASSET_ID} (resolved by location)\n`);

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
    fail("Direct EVM call", "EVM account has no WETH.");
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
          `Gas cost: ${ethers.formatEther(gasCost)} WETH (${receipt!.gasUsed} gas)`
        );
      } else {
        fail("Direct EVM call charges EVM source", "Balance unchanged");
      }
    } catch (err: any) {
      fail("Direct EVM call", `Error: ${err.message}`);
    }
  }

  // ─────────────────────────────────────────────────────────────────
  // TEST 2: dispatchWithFeePayer(proxy(pureProxy, EVM::call))
  //   Controller wraps proxy+EVM in dispatcher — gas charged to controller
  // ─────────────────────────────────────────────────────────────────

  console.log(
    "\n── Test 2: dispatchWithFeePayer(proxy(EVM::call)) ──\n"
  );

  try {
    console.log("  Creating pureProxy for Alice...");
    const pureProxy = await createPureProxy(api, alice);
    console.log(`  PureProxy: ${pureProxy}`);
    const pureProxyEvmAddr = substrateToEvmAddress(pureProxy);

    const aliceWethBefore = await getTokenBalance(api, aliceAddress, WETH_ASSET_ID);
    const aliceHdxBefore = await getTokenBalance(api, aliceAddress, HDX_ASSET_ID);

    const evmCall = makeEvmCall(api, pureProxyEvmAddr);
    const proxyCall = api.tx.proxy.proxy(pureProxy, null, evmCall);
    const wrappedCall = api.tx.dispatcher.dispatchWithFeePayer(proxyCall);

    console.log("  Sending dispatchWithFeePayer(proxy(EVM::call))...");
    const { events } = await sendAndWait(wrappedCall, alice, api);

    const proxyExecuted = events.find(
      ({ event }: any) =>
        event.section === "proxy" && event.method === "ProxyExecuted"
    );
    if (proxyExecuted) {
      console.log(`  ProxyExecuted: ${JSON.stringify(proxyExecuted.event.data[0].toJSON())}`);
    }

    const aliceWethAfter = await getTokenBalance(api, aliceAddress, WETH_ASSET_ID);
    const aliceHdxAfter = await getTokenBalance(api, aliceAddress, HDX_ASSET_ID);
    const wethCharged = aliceWethBefore - aliceWethAfter;
    const hdxCharged = aliceHdxBefore - aliceHdxAfter;

    if (hdxCharged > 0n || wethCharged > 0n) {
      pass(
        "dispatchWithFeePayer(proxy(EVM::call)) charges controller",
        `Alice charged: HDX=${hdxCharged}, WETH=${wethCharged}`
      );
    } else {
      fail(
        "dispatchWithFeePayer(proxy(EVM::call)) charges controller",
        `Alice balance unchanged. HDX=${hdxCharged}, WETH=${wethCharged}`
      );
    }
  } catch (err: any) {
    fail("dispatchWithFeePayer(proxy(EVM::call))", `Error: ${err.message}`);
  }

  // ─────────────────────────────────────────────────────────────────
  // TEST 3: batchAll([..., dispatchWithFeePayer(proxy(EVM::call))])
  //   The real-world UI pattern: batch wrapping the dispatcher call
  // ─────────────────────────────────────────────────────────────────

  console.log(
    "\n── Test 3: batchAll([dispatchWithFeePayer(proxy(EVM::call))]) ──\n"
  );

  try {
    console.log("  Creating pureProxy for Alice...");
    const pureProxy = await createPureProxy(api, alice);
    console.log(`  PureProxy: ${pureProxy}`);
    const pureProxyEvmAddr = substrateToEvmAddress(pureProxy);

    const proxyWethBefore = await getTokenBalance(api, pureProxy, WETH_ASSET_ID);
    const aliceWethBefore = await getTokenBalance(api, aliceAddress, WETH_ASSET_ID);
    const aliceHdxBefore = await getTokenBalance(api, aliceAddress, HDX_ASSET_ID);
    console.log(`  Proxy WETH: ${proxyWethBefore} (should be 0)`);

    const evmCall = makeEvmCall(api, pureProxyEvmAddr);
    const proxyCall = api.tx.proxy.proxy(pureProxy, null, evmCall);
    const dispatcherCall = api.tx.dispatcher.dispatchWithFeePayer(proxyCall);
    const remarkCall = api.tx.system.remark("0xdeadbeef");
    const batchCall = api.tx.utility.batchAll([remarkCall, dispatcherCall]);

    console.log("  Sending batchAll([remark, dispatchWithFeePayer(proxy(EVM::call))])...");
    const { events } = await sendAndWait(batchCall, alice, api);

    const proxyExecuted = events.find(
      ({ event }: any) =>
        event.section === "proxy" && event.method === "ProxyExecuted"
    );
    if (proxyExecuted) {
      console.log(`  ProxyExecuted: ${JSON.stringify(proxyExecuted.event.data[0].toJSON())}`);
    }

    const proxyWethAfter = await getTokenBalance(api, pureProxy, WETH_ASSET_ID);
    const aliceWethAfter = await getTokenBalance(api, aliceAddress, WETH_ASSET_ID);
    const aliceHdxAfter = await getTokenBalance(api, aliceAddress, HDX_ASSET_ID);
    const wethCharged = aliceWethBefore - aliceWethAfter;
    const hdxCharged = aliceHdxBefore - aliceHdxAfter;

    if (proxyWethBefore === 0n && proxyWethAfter === 0n && (hdxCharged > 0n || wethCharged > 0n)) {
      pass(
        "batchAll + dispatchWithFeePayer works (proxy has 0 WETH)",
        `Alice charged: HDX=${hdxCharged}, WETH=${wethCharged}`
      );
    } else if (hdxCharged > 0n || wethCharged > 0n) {
      pass(
        "batchAll + dispatchWithFeePayer charges controller",
        `Alice charged: HDX=${hdxCharged}, WETH=${wethCharged}`
      );
    } else {
      fail(
        "batchAll + dispatchWithFeePayer",
        `Alice balance unchanged. HDX=${hdxCharged}, WETH=${wethCharged}`
      );
    }
  } catch (err: any) {
    fail("batchAll + dispatchWithFeePayer", `Error: ${err.message}`);
  }

  // ─────────────────────────────────────────────────────────────────
  // TEST 4: batchAll([dispatchWithExtraGas, bindEvmAddress, dispatchWithFeePayer(proxy(EVM::call))])
  //   Full lark-style pattern matching the real UI flow
  // ─────────────────────────────────────────────────────────────────

  console.log(
    "\n── Test 4: Full lark pattern (dispatchWithExtraGas + bind + dispatchWithFeePayer) ──\n"
  );

  try {
    console.log("  Creating pureProxy for Alice...");
    const pureProxy = await createPureProxy(api, alice);
    console.log(`  PureProxy: ${pureProxy}`);
    const pureProxyEvmAddr = substrateToEvmAddress(pureProxy);

    const aliceWethBefore = await getTokenBalance(api, aliceAddress, WETH_ASSET_ID);
    const aliceHdxBefore = await getTokenBalance(api, aliceAddress, HDX_ASSET_ID);

    const transferCall = api.tx.currencies.transfer(pureProxy, HDX_ASSET_ID, 1_000_000_000_000);
    const dispatchExtraGas = api.tx.dispatcher.dispatchWithExtraGas(transferCall, 1_000_000);

    const bindCall = api.tx.proxy.proxy(pureProxy, "Any", api.tx.evmAccounts.bindEvmAddress());

    const evmCall = makeEvmCall(api, pureProxyEvmAddr);
    const proxyEvmCall = api.tx.proxy.proxy(pureProxy, "Any", evmCall);
    const dispatchFeePayer = api.tx.dispatcher.dispatchWithFeePayer(proxyEvmCall);

    const fullBatch = api.tx.utility.batchAll([
      dispatchExtraGas,
      bindCall,
      dispatchFeePayer,
    ]);

    console.log("  Sending full lark-style batchAll...");
    const { events } = await sendAndWait(fullBatch, alice, api);

    const proxyEvents = events.filter(
      ({ event }: any) =>
        event.section === "proxy" && event.method === "ProxyExecuted"
    );
    console.log(`  ProxyExecuted events: ${proxyEvents.length}`);
    for (const pe of proxyEvents) {
      console.log(`    ${JSON.stringify(pe.event.data[0].toJSON())}`);
    }

    const aliceWethAfter = await getTokenBalance(api, aliceAddress, WETH_ASSET_ID);
    const aliceHdxAfter = await getTokenBalance(api, aliceAddress, HDX_ASSET_ID);
    const wethCharged = aliceWethBefore - aliceWethAfter;
    const hdxCharged = aliceHdxBefore - aliceHdxAfter;

    if (hdxCharged > 0n || wethCharged > 0n) {
      pass(
        "Full lark pattern charges controller",
        `Alice charged: HDX=${hdxCharged}, WETH=${wethCharged}`
      );
    } else {
      fail(
        "Full lark pattern",
        `Alice balance unchanged. HDX=${hdxCharged}, WETH=${wethCharged}`
      );
    }
  } catch (err: any) {
    fail("Full lark pattern", `Error: ${err.message}`);
  }

  // ─────────────────────────────────────────────────────────────────
  // TEST 5: dispatchWithFeePayer fails when controller has no funds
  // ─────────────────────────────────────────────────────────────────

  console.log(
    "\n── Test 5: dispatchWithFeePayer fails when controller has no funds ──\n"
  );

  const dave = keyring.addFromUri("//Dave");
  const daveAddress = dave.address;
  console.log(`  Dave (no-funds controller): ${daveAddress}`);

  try {
    console.log("  Creating pureProxy for Dave...");
    const davePureProxy = await createPureProxy(api, dave);
    const davePureEvmAddr = substrateToEvmAddress(davePureProxy);

    const daveWeth = await getTokenBalance(api, daveAddress, WETH_ASSET_ID);
    console.log(`  Dave WETH: ${daveWeth}`);

    const evmCall = makeEvmCall(api, davePureEvmAddr);
    const proxyCall = api.tx.proxy.proxy(davePureProxy, null, evmCall);
    const wrappedCall = api.tx.dispatcher.dispatchWithFeePayer(proxyCall);

    console.log("  Sending dispatchWithFeePayer as Dave (no WETH)...");
    const { events } = await sendAndWait(wrappedCall, dave, api);

    const proxyExecuted = events.find(
      ({ event }: any) =>
        event.section === "proxy" && event.method === "ProxyExecuted"
    );

    if (proxyExecuted) {
      const resultJson = proxyExecuted.event.data[0].toJSON();
      const isErr = resultJson && typeof resultJson === "object" && "err" in resultJson;

      if (isErr) {
        pass(
          "dispatchWithFeePayer fails with no-funds controller",
          `Inner error: ${JSON.stringify(resultJson)}`
        );
      } else {
        fail(
          "dispatchWithFeePayer fails with no-funds controller",
          `Expected error but got: ${JSON.stringify(resultJson)}`
        );
      }
    } else {
      fail("dispatchWithFeePayer fails with no-funds controller", "No ProxyExecuted event");
    }
  } catch (err: any) {
    if (
      err.message.includes("BalanceLow") ||
      err.message.includes("WithdrawFailed") ||
      err.message.includes("Inability to pay") ||
      err.message.includes("1010")
    ) {
      pass(
        "dispatchWithFeePayer fails with no-funds controller",
        `Expected error: ${err.message.slice(0, 120)}`
      );
    } else {
      fail(
        "dispatchWithFeePayer fails with no-funds controller",
        `Error: ${err.message.slice(0, 120)}`
      );
    }
  }

  // ─────────────────────────────────────────────────────────────────
  // TEST 6: Non-EVM proxy call works normally without dispatcher wrapper
  // ─────────────────────────────────────────────────────────────────

  console.log("\n── Test 6: Non-EVM proxy call works normally ──\n");

  try {
    try {
      await sendAndWait(
        api.tx.proxy.addProxy(aliceAddress, "Any", 0),
        bob,
        api
      );
    } catch {}

    const aliceHdxBefore = await getTokenBalance(api, aliceAddress, HDX_ASSET_ID);

    const remarkCall = api.tx.system.remark("0x1234");
    const proxyRemarkCall = api.tx.proxy.proxy(bobAddress, null, remarkCall);
    await sendAndWait(proxyRemarkCall, alice, api);

    const aliceHdxAfter = await getTokenBalance(api, aliceAddress, HDX_ASSET_ID);
    const remarkFee = aliceHdxBefore - aliceHdxAfter;

    if (remarkFee >= 0n) {
      pass(
        "Non-EVM proxy call works normally",
        `Standard substrate fee: ${remarkFee} (${remarkFee / UNITS} UNITS HDX)`
      );
    } else {
      fail("Non-EVM proxy call works normally", `Unexpected balance increase: ${remarkFee}`);
    }
  } catch (err: any) {
    fail("Non-EVM proxy call works normally", `Error: ${err.message}`);
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
