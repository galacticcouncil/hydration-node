// Replace the runtime wasm in a forked chainspec with a freshly-built local
// wasm. Faster than running a full preauth + applyAuthorizedUpgrade dance
// when iterating on the fork — the parachain comes up at the new spec from
// block 1 (no upgrade extrinsic required).
//
// Usage:
//   node inject-wasm.mjs                          # uses defaults below
//   WASM=/path/to.wasm SPEC=data/foo.json node inject-wasm.mjs
//
// Note: zombienet caches a derived spec in data/local-2034-rococo-local.json
// after the first launch — clear data/ before restart so the new wasm is
// picked up. (See `clean-fork.sh` if you have one, or rm -rf the data
// subdirs before `npm run zombie:init`.)

import fs from 'fs';

const WASM_PATH =
	process.env.WASM ||
	'../../target/release/wbuild/hydradx-runtime/hydradx_runtime.compact.compressed.wasm';
const SPEC_PATH = process.env.SPEC || 'data/forked-chainspec.json';
const CODE_KEY = '0x3a636f6465'; // sp_storage::well_known_keys::CODE = b":code"

const wasm = fs.readFileSync(WASM_PATH);
const spec = JSON.parse(fs.readFileSync(SPEC_PATH, 'utf8'));

const oldLen = (spec.genesis.raw.top[CODE_KEY] || '').length;
spec.genesis.raw.top[CODE_KEY] = '0x' + wasm.toString('hex');

fs.writeFileSync(SPEC_PATH, JSON.stringify(spec));

console.log(`wasm:  ${WASM_PATH} (${wasm.length} bytes)`);
console.log(`spec:  ${SPEC_PATH}`);
console.log(`:code: ${oldLen} -> ${spec.genesis.raw.top[CODE_KEY].length} hex chars`);
