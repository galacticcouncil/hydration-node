/**
 * patch-chainspec.js
 *
 * Patches a plain HydraDX chain spec JSON to add:
 *   - WETH (asset 20) registration
 *   - Native HDX balances for Alice & Bob (for governance deposits)
 *   - TC members (Alice, Bob, Charlie)
 *   - Token balances (WETH for test accounts)
 *   - Parachain ID override to 2032
 *
 * Uses text-level manipulation for large numbers to avoid
 * JavaScript's Number precision loss (scientific notation).
 *
 * Usage: node patch-chainspec.js <chainspec-plain.json>
 */

const fs = require("fs");
const path = process.argv[2];
if (!path) {
  console.error("Usage: node patch-chainspec.js <chainspec-plain.json>");
  process.exit(1);
}

// Read as text to preserve numeric precision
let text = fs.readFileSync(path, "utf8");

// Parse for structural edits (small numbers are safe)
const spec = JSON.parse(text);
const genesis = spec.genesis.runtimeGenesis.patch;

// Ensure WETH (asset 20) is in registeredAssets
const hasWeth = genesis.assetRegistry.registeredAssets.some(
  (a) => a[0] === 20
);
if (!hasWeth) {
  genesis.assetRegistry.registeredAssets.push([
    20,
    [69, 116, 104, 101, 114, 101, 117, 109], // 'Ethereum'
    1000000000000, // existential deposit
    [87, 69, 84, 72], // 'WETH'
    18,
    null,
    true,
  ]);
}

const alice = "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY";
const bob = "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty";
const charlie = "5FLSigC9HGRKVhB9FiEo4Y3koPsNmBmLJbpXg2mp1hXcS59Y";
const charlieEvm = "5DdcCSDqrt3ThGfhqr63psaauSd1HZPpXEmWcNdiggzkXehL";

// Native HDX balances: replace existing entries with enough for governance.
// Need >36% of total issuance as support for the referendum to pass immediately.
// Give Alice 1B UNITS so her vote dominates total issuance.
if (!genesis.balances) genesis.balances = { balances: [] };
const TARGET_NATIVE = "__NATIVE_AMOUNT__";
for (const addr of [alice, bob]) {
  const idx = genesis.balances.balances.findIndex((b) => b[0] === addr);
  if (idx >= 0) {
    genesis.balances.balances[idx] = [addr, TARGET_NATIVE];
  } else {
    genesis.balances.balances.push([addr, TARGET_NATIVE]);
  }
}

// TC members
if (!genesis.technicalCommittee) genesis.technicalCommittee = {};
genesis.technicalCommittee.members = [alice, bob, charlie];

// Token balances: use string amounts for WETH to avoid scientific notation
// We'll add them as strings and fix up after JSON.stringify
const wethAmount = "100000000000000000000000"; // 100,000 WETH (10^23)
const tokenEntries = [
  [charlieEvm, 20],
  [alice, 20],
  [bob, 20],
];
for (const [addr, assetId] of tokenEntries) {
  const exists = genesis.tokens.balances.some(
    (b) => b[0] === addr && b[1] === assetId
  );
  if (!exists) {
    // Push with a placeholder that we'll replace
    genesis.tokens.balances.push([addr, assetId, "__WETH_AMOUNT__"]);
  }
}

// Override parachain ID to match zombienet config
genesis.parachainInfo = { parachainId: 2032 };
spec.para_id = 2032;

// Serialize and replace placeholders with unquoted large numbers
let output = JSON.stringify(spec, null, 2);

// Replace string placeholders with raw numbers
const nativeAmount = "1000000000000000000000"; // 1B UNITS (10^21)
output = output.replace(/"__WETH_AMOUNT__"/g, wethAmount);
output = output.replace(/"__NATIVE_AMOUNT__"/g, nativeAmount);

// Also fix any existing WETH amounts that got mangled to scientific notation
output = output.replace(/1e\+23/g, wethAmount);

fs.writeFileSync(path, output);
console.log("  Patched: WETH asset, balances, TC members, parachain ID 2032");
