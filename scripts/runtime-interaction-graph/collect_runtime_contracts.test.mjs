import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

import {
	accountKey20Address,
	addressValues,
	collectRuntimeConfigurations,
	deduplicateConfigurations,
	erc20AssetAddress,
	isErc20Asset,
	options,
	outputPayload,
	requiredOption,
	requiredRuntimeQueries,
	runtimeConfiguration,
	validateInputPayload,
	validateSubstrateIdentity,
} from "./collect_runtime_contracts.mjs";


const GENESIS = `0x${"11".repeat(32)}`;
const ADDRESS = "0x1111111111111111111111111111111111111111";


function codec(value) {
	return { toJSON: () => value };
}


function runtimeQueries(overrides = {}) {
	const gigaHdxPoolContract = async () => codec(ADDRESS);
	const flashMinter = async () => codec(null);
	const borrowingContract = async () => codec({ value: ADDRESS });
	const assets = async () => codec(null);
	assets.entries = async () => [
		[{ args: [{ toString: () => "7" }] }, codec({ assetType: "Erc20" })],
		[{ args: [{ toString: () => "8" }] }, codec({ assetType: "Token" })],
		[{ args: [{ toString: () => "9" }] }, codec({ assetType: "Erc20" })],
	];
	const assetLocations = async (assetId) => codec(assetId.toString() === "7" ? {
		parents: 0,
		interior: { x1: [{ accountKey20: { network: null, key: ADDRESS } }] },
	} : null);
	return {
		query: {
			gigaHdx: { gigaHdxPoolContract },
			hsm: { flashMinter },
			liquidation: { borrowingContract },
			assetRegistry: { assets, assetLocations },
			...overrides,
		},
	};
}


test("runtime collector requires explicit unique options", () => {
	const parsed = options(["--input", "in.json", "--output", "out.json"]);
	assert.equal(requiredOption(parsed, "--input"), "in.json");
	assert.throws(() => requiredOption(parsed, "--rpc"), /missing required option/);
	assert.throws(() => options(["--input", "a", "--input", "b"]), /duplicate option/);
});


test("runtime collector extracts exact nested addresses only", () => {
	assert.deepEqual(addressValues({ values: [
		"0x1111111111111111111111111111111111111111",
		"prefix0x2222222222222222222222222222222222222222",
	] }), ["0x1111111111111111111111111111111111111111"]);
});


test("runtime query validation should fail when required API is missing", () => {
	const at = runtimeQueries({ hsm: {} });
	assert.throws(() => requiredRuntimeQueries(at), /at\.query\.hsm\.flashMinter/);
});


test("runtime collector should report complete query coverage", async () => {
	const payload = { rpc_snapshot: { chain_id: 42 } };
	const { configurations, queryCoverage } = await collectRuntimeConfigurations(payload, runtimeQueries());
	assert.equal(queryCoverage.required_query_count, 5);
	assert.equal(queryCoverage.available_query_count, 5);
	assert.equal(queryCoverage.collected_configuration_count, 3);
	assert.equal(queryCoverage.unique_configuration_count, 3);
	assert.deepEqual(queryCoverage.queries["hsm.flashMinter"], {
		available: true,
		calls: 1,
		records: 0,
		configurations: 0,
	});
	assert.deepEqual(queryCoverage.queries["assetRegistry.assets"], {
		available: true,
		calls: 1,
		records: 3,
		configurations: 0,
		erc20_assets: 2,
	});
	assert.equal(queryCoverage.queries["assetRegistry.assetLocations"].calls, 2);
	assert.equal(queryCoverage.queries["assetRegistry.assetLocations"].configurations, 1);
	assert.equal(configurations.length, 3);
});


test("asset registry collector should extract only Erc20 AccountKey20 locations", () => {
	const address = ADDRESS;
	const asciiName = "0x44414920284163616c6120576f726d686f6c6529";
	const location = { parents: 0, interior: { x1: [{ accountKey20: { network: null, key: address } }] } };
	assert.equal(isErc20Asset({ assetType: "Erc20", name: asciiName }), true);
	assert.equal(accountKey20Address(location), address);
	assert.equal(erc20AssetAddress({ assetType: "Erc20", name: asciiName }, location), address);
	assert.equal(erc20AssetAddress({ assetType: "Token", name: asciiName }, location), null);
	assert.equal(erc20AssetAddress({ assetType: "Erc20", name: asciiName }, {
		parents: 0, interior: { x1: [{ generalKey: { data: asciiName } }] },
	}), null);
	assert.equal(erc20AssetAddress({ assetType: "Erc20", name: asciiName }, {
		parents: 0, interior: { x2: [{ accountKey20: { key: address } }] },
	}), null);
});


test("runtime collector should enforce expected Substrate identity", () => {
	const snapshot = { genesis_hash: GENESIS, spec_name: "hydradx" };
	assert.doesNotThrow(() => validateSubstrateIdentity(snapshot, GENESIS.toUpperCase().replace("0X", "0x"),
		"hydradx"));
	assert.throws(() => validateSubstrateIdentity(snapshot, `0x${"22".repeat(32)}`, "hydradx"),
		/does not match expected genesis/);
	assert.throws(() => validateSubstrateIdentity(snapshot, GENESIS, "other"), /does not match expected spec/);
});


test("runtime collector rejects legacy and mismatched snapshots", () => {
	assert.throws(() => validateInputPayload({ schema_version: 1 }), /schema_version 2/);
	const payload = {
		schema_version: 2,
		collection_provenance: { descriptor_sha256: "hash" },
		enrichment_provenance: { input_sha256: "hash" },
		rpc_snapshot: { chain_id: 42, block_hash: "0xabc", block_number: 10 },
		observations: [{ chain_id: 42, address: "0x1111111111111111111111111111111111111111",
			chain_address_id: "eip155:42:0x1111111111111111111111111111111111111111" }],
	};
	assert.doesNotThrow(() => validateInputPayload(payload, 10));
	assert.throws(() => validateInputPayload(payload, 11), /does not match EVM block/);
});


test("CI contract fixture should satisfy pinned snapshot validation", () => {
	const payload = JSON.parse(fs.readFileSync(new URL("./fixtures/ci-contracts.json", import.meta.url)));
	assert.doesNotThrow(() => validateInputPayload(payload, 13161523));
	assert.doesNotThrow(() => validateSubstrateIdentity(payload.substrate_snapshot,
		payload.chain_context.substrate_genesis_hash, payload.chain_context.substrate_spec_name));
	assert.equal(payload.runtime_collection_provenance.query_coverage.required_query_count, 5);
	assert.equal(payload.runtime_collection_provenance.query_coverage.available_query_count, 5);
});


test("runtime output preserves canonical chain identity and deduplicates configuration", () => {
	const payload = { rpc_snapshot: { chain_id: 42, block_hash: "0xevm" } };
	const snapshot = { genesis_hash: GENESIS, block_hash: "0xsubstrate", block_number: 10,
		spec_name: "hydradx" };
	const configuration = runtimeConfiguration(payload, "pallet:hsm", "hsm.flashMinter",
		"0x1111111111111111111111111111111111111111");
	assert.equal(configuration.chain_address_id,
		"eip155:42:0x1111111111111111111111111111111111111111");
	assert.equal(deduplicateConfigurations([configuration, configuration]).length, 1);
	const queryCoverage = { required_query_count: 5, available_query_count: 5, queries: {} };
	const result = outputPayload(payload, [configuration, configuration], snapshot, "input-hash",
		{ node: "v1", polkadotApi: "2", collectorSha256: "collector" }, queryCoverage);
	assert.deepEqual(result.chain_context, {
		evm_chain_id: 42,
		evm_block_hash: "0xevm",
		substrate_genesis_hash: GENESIS,
		substrate_block_hash: "0xsubstrate",
		substrate_spec_name: "hydradx",
		block_number: 10,
	});
	assert.equal(result.runtime_collection_provenance.input_sha256, "input-hash");
	assert.equal(result.runtime_collection_provenance.collector_sha256, "collector");
	assert.equal(result.runtime_collection_provenance.query_coverage, queryCoverage);
	assert.equal(result.runtime_configurations.length, 1);
});
