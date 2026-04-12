/**
 * test-gas-estimation.js
 *
 * Demonstrates the gas estimation issue described in:
 *   https://github.com/galacticcouncil/hydration-node/issues/1133
 *
 * Deploys a DepositProxy contract (mimicking DecentralPool) that makes
 * 6 external calls per deposit. Then compares eth_estimateGas results
 * with actual gas usage.
 *
 * WITHOUT rpc-binary-search-estimate:
 *   eth_estimateGas returns ~15-45M (near block gas limit)
 *
 * WITH rpc-binary-search-estimate:
 *   eth_estimateGas returns ~200-400K (close to actual usage)
 *
 * Usage: npx hardhat run scripts/test-gas-estimation.js --network local
 */

const hre = require("hardhat");

function formatGas(gas) {
  return Number(gas).toLocaleString();
}

async function main() {
  console.log("");
  console.log("=".repeat(64));
  console.log("  HydraDX Gas Estimation Test");
  console.log("  Issue #1133: Fix EVM gas estimation");
  console.log("  https://github.com/galacticcouncil/hydration-node/issues/1133");
  console.log("=".repeat(64));

  const [deployer] = await hre.ethers.getSigners();
  const provider = hre.ethers.provider;

  const balance = await provider.getBalance(deployer.address);
  console.log(`\n  Account:  ${deployer.address}`);
  console.log(`  Balance:  ${hre.ethers.formatEther(balance)} WETH`);

  if (balance === 0n) {
    console.error("\n  ERROR: Account has no WETH. Run 'npm run setup' first.\n");
    process.exit(1);
  }

  // ── Step 1: Deploy contracts ──────────────────────────────────────

  console.log("\n── Step 1: Deploy Contracts ─────────────────────────────────\n");

  // NOTE: We use explicit gasLimit for deployments because the gas estimation
  // bug (issue #1133) causes eth_estimateGas to return wildly inflated values
  // that exceed the block gas limit, making deployment impossible without a cap.
  const Token = await hre.ethers.getContractFactory("MockStablecoin");
  const token = await Token.deploy({ gasLimit: 5_000_000 });
  await token.waitForDeployment();
  const tokenAddr = await token.getAddress();
  console.log(`  MockStablecoin: ${tokenAddr}`);

  const Proxy = await hre.ethers.getContractFactory("DepositProxy");
  const proxy = await Proxy.deploy(tokenAddr, { gasLimit: 15_000_000 });
  await proxy.waitForDeployment();
  const proxyAddr = await proxy.getAddress();
  console.log(`  DepositProxy:   ${proxyAddr}`);

  // ── Step 2: Approve tokens ────────────────────────────────────────

  const depositAmount = hre.ethers.parseEther("10"); // 10 MUSD
  const approveTx = await token.approve(proxyAddr, depositAmount, { gasLimit: 500_000 });
  await approveTx.wait();
  console.log(`\n  Approved ${hre.ethers.formatEther(depositAmount)} MUSD for DepositProxy`);

  // Build the deposit() calldata
  const depositData = proxy.interface.encodeFunctionData("deposit", [
    depositAmount,
  ]);

  // ── Step 3: Gas estimation WITHOUT gas limit ──────────────────────
  //
  // This is what wallets like Talisman do: call eth_estimateGas with
  // no gas field, so the node uses the full block gas limit as ceiling.

  console.log("\n── Step 2: eth_estimateGas WITHOUT gas limit ────────────────");
  console.log("  (Simulates wallet behavior: Talisman, Rabby, etc.)\n");

  let estimateNoLimit;
  try {
    const raw = await provider.send("eth_estimateGas", [
      {
        from: deployer.address,
        to: proxyAddr,
        data: depositData,
        value: "0x0",
      },
    ]);
    estimateNoLimit = BigInt(raw);
    console.log(`  Result: ${formatGas(estimateNoLimit)} gas`);
  } catch (err) {
    console.log(`  ERROR: ${err.message}`);
    estimateNoLimit = null;
  }

  // ── Step 4: Gas estimation WITH gas limit ─────────────────────────
  //
  // When a gas cap is provided, the executor uses it as ceiling instead
  // of the block gas limit. This gives a better estimate even without
  // binary search, but wallets don't always do this.

  console.log("\n── Step 3: eth_estimateGas WITH gas=500K cap ────────────────");
  console.log("  (What happens when the caller provides a reasonable cap)\n");

  let estimateWithLimit;
  try {
    const raw = await provider.send("eth_estimateGas", [
      {
        from: deployer.address,
        to: proxyAddr,
        data: depositData,
        value: "0x0",
        gas: "0x" + (500000).toString(16),
      },
    ]);
    estimateWithLimit = BigInt(raw);
    console.log(`  Result: ${formatGas(estimateWithLimit)} gas`);
  } catch (err) {
    console.log(`  ERROR: ${err.message}`);
    estimateWithLimit = null;
  }

  // ── Step 5: Actual transaction ────────────────────────────────────
  //
  // Execute deposit() with explicit gasLimit to measure real consumption.

  console.log("\n── Step 4: Actual Transaction (gasLimit=500K) ───────────────\n");

  const tx = await proxy.deposit(depositAmount, { gasLimit: 500_000 });
  const receipt = await tx.wait();
  const actualGas = receipt.gasUsed;
  console.log(`  Transaction hash: ${receipt.hash}`);
  console.log(`  Actual gas used:  ${formatGas(actualGas)}`);

  // ── Results ───────────────────────────────────────────────────────

  const block = await provider.getBlock("latest");
  const blockGasLimit = block.gasLimit;

  console.log("\n" + "=".repeat(64));
  console.log("  RESULTS");
  console.log("=".repeat(64));
  console.log(`\n  Block gas limit:       ${formatGas(blockGasLimit)}`);
  console.log(`  Actual gas used:       ${formatGas(actualGas)}`);

  if (estimateNoLimit !== null) {
    const ratio = Number(estimateNoLimit) / Number(actualGas);
    console.log(
      `  Estimate (no limit):   ${formatGas(estimateNoLimit)}  (${ratio.toFixed(1)}x actual)`
    );
  }

  if (estimateWithLimit !== null) {
    const ratio2 = Number(estimateWithLimit) / Number(actualGas);
    console.log(
      `  Estimate (500K cap):   ${formatGas(estimateWithLimit)}  (${ratio2.toFixed(1)}x actual)`
    );
  }

  // ── Diagnosis ─────────────────────────────────────────────────────

  console.log("\n" + "-".repeat(64));

  if (estimateNoLimit !== null) {
    const ratio = Number(estimateNoLimit) / Number(actualGas);

    if (ratio > 10) {
      // Binary search is NOT enabled
      console.log("  STATUS: rpc-binary-search-estimate is NOT enabled");
      console.log("");
      console.log(`  The estimate (${formatGas(estimateNoLimit)}) is ${ratio.toFixed(0)}x the actual usage.`);

      if (estimateNoLimit > blockGasLimit) {
        console.log(`  It EXCEEDS the block gas limit (${formatGas(blockGasLimit)}).`);
        console.log("");
        console.log("  This is exactly what causes the Talisman/wallet failure:");
        console.log("    1. Wallet calls eth_estimateGas -> gets ~" + formatGas(estimateNoLimit));
        console.log("    2. Wallet sets tx gasLimit to ~" + formatGas(estimateNoLimit));
        console.log('    3. Node rejects: "exceeds block gas limit"');
        console.log("    4. User cannot send the transaction");
      } else {
        console.log("");
        console.log("  While it doesn't exceed the block gas limit, the estimate is");
        console.log("  wildly inaccurate. Users pay for far more gas than needed.");
      }

      console.log("");
      console.log("  FIX: In the workspace root Cargo.toml, change fc-rpc to:");
      console.log('    fc-rpc = { ..., features = ["rpc-binary-search-estimate"] }');
      console.log("");
      console.log("  Then rebuild and re-run this test to see the improvement.");
    } else if (ratio < 3) {
      // Binary search IS enabled
      console.log("  STATUS: rpc-binary-search-estimate IS enabled");
      console.log("");
      console.log(`  The estimate (${formatGas(estimateNoLimit)}) is only ${ratio.toFixed(1)}x the actual usage.`);
      console.log("  This is a tight, accurate estimate. Wallets will work correctly.");
      console.log("");
      console.log("  The fix is working as expected!");
    } else {
      console.log(`  STATUS: Estimate is ${ratio.toFixed(1)}x actual - moderate overestimate.`);
    }
  }

  console.log("-".repeat(64));
  console.log("");
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
