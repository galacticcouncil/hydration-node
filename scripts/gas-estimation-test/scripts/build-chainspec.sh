#!/bin/bash
# Build a patched chain spec that includes WETH (asset 20) registration,
# pre-funds accounts, sets TC members, and enables testnet governance tracks.
#
# Usage: ./scripts/build-chainspec.sh
# Output: chainspec-raw.json (use with zombienet)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
HYDRADX="$ROOT_DIR/target/release/hydradx"
OUTPUT_DIR="$SCRIPT_DIR/.."

if [ ! -f "$HYDRADX" ]; then
  echo "ERROR: $HYDRADX not found. Build the node first: cargo build --release"
  exit 1
fi

echo "=== Building patched chain spec with WETH + testnet mode ==="

# Step 1: Export plain chain spec
echo "[1/4] Exporting plain chain spec..."
"$HYDRADX" build-spec --chain local --disable-default-bootnode 2>/dev/null > "$OUTPUT_DIR/chainspec-plain.json"

# Step 2: Patch genesis config (assets, balances, TC members)
# Uses a separate .js file to handle large numbers correctly
echo "[2/4] Patching genesis config..."
node "$SCRIPT_DIR/patch-chainspec.js" "$OUTPUT_DIR/chainspec-plain.json"

# Step 3: Build raw chain spec
echo "[3/4] Building raw chain spec..."
"$HYDRADX" build-spec \
  --chain "$OUTPUT_DIR/chainspec-plain.json" \
  --raw \
  --disable-default-bootnode \
  2>/dev/null > "$OUTPUT_DIR/chainspec-raw.json"

if [ ! -s "$OUTPUT_DIR/chainspec-raw.json" ]; then
  echo "ERROR: Raw chain spec is empty. Re-running with errors visible:"
  "$HYDRADX" build-spec \
    --chain "$OUTPUT_DIR/chainspec-plain.json" \
    --raw \
    --disable-default-bootnode
  exit 1
fi

# Step 4: Inject raw storage values into chain spec
echo "[4/4] Injecting raw storage (IsTestnet + ContractDeployer)..."
node -e "
const fs = require('fs');
const { xxhashAsHex, blake2AsHex } = require('@polkadot/util-crypto');
const { hexToU8a } = require('@polkadot/util');

const spec = JSON.parse(fs.readFileSync('$OUTPUT_DIR/chainspec-raw.json', 'utf8'));

// 1) Parameters::IsTestnet = true
const isTestnetKey = xxhashAsHex('Parameters', 128) + xxhashAsHex('IsTestnet', 128).slice(2);
spec.genesis.raw.top[isTestnetKey] = '0x01';
console.log('  IsTestnet key: ' + isTestnetKey);
console.log('  Parameters::IsTestnet = true (enables fast governance tracks)');

// 2) EVMAccounts::ContractDeployer for deployer address
// Storage key: twox_128('EVMAccounts') ++ twox_128('ContractDeployer') ++ blake2_128_concat(address)
// blake2_128_concat = blake2_128(data) ++ data
const evmAddress = '0xC19A2970A13ac19898c47d59Cbd0278D428EBC7c';
const addressBytes = hexToU8a(evmAddress);
const addressHash = blake2AsHex(addressBytes, 128).slice(2); // 16 bytes = 32 hex chars, remove 0x
const addressHex = Buffer.from(addressBytes).toString('hex');
const deployerKey = xxhashAsHex('EVMAccounts', 128)
  + xxhashAsHex('ContractDeployer', 128).slice(2)
  + addressHash
  + addressHex;
// Value for () is empty SCALE encoding
spec.genesis.raw.top[deployerKey] = '0x';
console.log('  Deployer key: ' + deployerKey);
console.log('  ContractDeployer whitelisted: ' + evmAddress);

fs.writeFileSync('$OUTPUT_DIR/chainspec-raw.json', JSON.stringify(spec, null, 2));
console.log('  Written to chainspec-raw.json');
"

# Cleanup
rm -f "$OUTPUT_DIR/chainspec-plain.json"

echo ""
echo "=== Done ==="
echo "Raw chain spec: $OUTPUT_DIR/chainspec-raw.json"
