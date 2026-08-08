#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";


const ADDRESS = /^0x[0-9a-fA-F]{40}$/;
const HASH_256 = /^0x[0-9a-fA-F]{64}$/;
const COLLECTOR_VERSION = 4;
const CONTRACT_QUERIES = [
	["gigaHdx", "gigaHdxPoolContract", "pallet:gigahdx"],
	["hsm", "flashMinter", "pallet:hsm"],
	["liquidation", "borrowingContract", "pallet:liquidation"],
];
const ASSET_QUERIES = [
	["assetRegistry", "assets"],
	["assetRegistry", "assetLocations"],
];


export function options(argv) {
	const result = new Map();
	for (let index = 0; index < argv.length; index += 2) {
		const flag = argv[index];
		const value = argv[index + 1];
		if (!flag?.startsWith("--") || value === undefined || value.startsWith("--")) {
			throw new Error(`expected --flag value, received ${flag ?? "end of arguments"}`);
		}
		if (result.has(flag)) throw new Error(`duplicate option: ${flag}`);
		result.set(flag, value);
	}
	return result;
}


export function requiredOption(parsed, flag) {
	const value = parsed.get(flag);
	if (!value) throw new Error(`missing required option: ${flag}`);
	return value;
}


export function addressValues(value) {
	if (typeof value === "string") return ADDRESS.test(value) ? [value.toLowerCase()] : [];
	if (Array.isArray(value)) return value.flatMap(addressValues);
	if (value && typeof value === "object") return Object.values(value).flatMap(addressValues);
	return [];
}


function namedValue(value, name) {
	if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
	const normalized = name.replaceAll("_", "").toLowerCase();
	const entry = Object.entries(value).find(([key]) => key.replaceAll("_", "").toLowerCase() === normalized);
	return entry?.[1];
}


export function isErc20Asset(details) {
	const assetType = namedValue(details, "assetType");
	if (typeof assetType === "string") return assetType.toLowerCase() === "erc20";
	return assetType && typeof assetType === "object" && namedValue(assetType, "erc20") !== undefined;
}


export function accountKey20Address(assetLocation) {
	let location = assetLocation;
	if (Array.isArray(location) && location.length === 1) [location] = location;
	const interior = namedValue(location, "interior");
	const x1 = namedValue(interior, "x1");
	const junctions = Array.isArray(x1) ? x1 : (x1 ? [x1] : []);
	if (junctions.length !== 1) return null;
	const accountKey20 = namedValue(junctions[0], "accountKey20");
	const key = namedValue(accountKey20, "key");
	return typeof key === "string" && ADDRESS.test(key) ? key.toLowerCase() : null;
}


export function erc20AssetAddress(details, assetLocation) {
	return isErc20Asset(details) ? accountKey20Address(assetLocation) : null;
}


export function validateSubstrateIdentity(snapshot, expectedGenesisHash, expectedSpecName) {
	if (!HASH_256.test(expectedGenesisHash)) throw new Error(`invalid expected Substrate genesis hash: ${expectedGenesisHash}`);
	if (!expectedSpecName) throw new Error("expected Substrate spec name must not be empty");
	if (snapshot.genesis_hash.toLowerCase() !== expectedGenesisHash.toLowerCase()) {
		throw new Error(
			`Substrate genesis ${snapshot.genesis_hash} does not match expected genesis ${expectedGenesisHash}`
		);
	}
	if (snapshot.spec_name !== expectedSpecName) {
		throw new Error(`Substrate spec ${snapshot.spec_name} does not match expected spec ${expectedSpecName}`);
	}
}


export function validateInputPayload(payload, requestedBlockNumber = null) {
	if (payload.schema_version !== 2) throw new Error("contract snapshot must use schema_version 2");
	if (!payload.collection_provenance?.descriptor_sha256 || !payload.enrichment_provenance?.input_sha256) {
		throw new Error("contract snapshot has no verified collection and EVM enrichment provenance");
	}
	const snapshot = payload.rpc_snapshot;
	if (!snapshot?.chain_id || !snapshot.block_hash || !Number.isInteger(snapshot.block_number)) {
		throw new Error("contract snapshot has no canonical EVM chain and block identity");
	}
	if (!Array.isArray(payload.observations) || !payload.observations.length || payload.observations.some(
		(observation) => observation.chain_id !== snapshot.chain_id ||
			observation.chain_address_id !== `eip155:${snapshot.chain_id}:${observation.address}`
	)) {
		throw new Error("contract snapshot has missing or mixed-chain observations");
	}
	if (requestedBlockNumber !== null && snapshot.block_number !== requestedBlockNumber) {
		throw new Error(
			`requested Substrate block ${requestedBlockNumber} does not match EVM block ${snapshot.block_number}`
		);
	}
}


export function deduplicateConfigurations(configurations) {
	const unique = new Map();
	for (const configuration of configurations) unique.set(JSON.stringify(configuration), configuration);
	return [...unique.values()].sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right)));
}


export function runtimeConfiguration(payload, component, storage, address, extra = {}) {
	return {
		component,
		storage,
		...extra,
		chain_id: payload.rpc_snapshot.chain_id,
		address,
		chain_address_id: `eip155:${payload.rpc_snapshot.chain_id}:${address}`,
	};
}


function requiredQuery(at, section, method) {
	const query = at?.query?.[section]?.[method];
	if (typeof query !== "function") {
		throw new Error(`required runtime query API is missing: at.query.${section}.${method}`);
	}
	return query;
}


export function requiredRuntimeQueries(at) {
	const queries = Object.fromEntries([...CONTRACT_QUERIES, ...ASSET_QUERIES].map(([section, method]) => [
		`${section}.${method}`,
		requiredQuery(at, section, method),
	]));
	if (typeof queries["assetRegistry.assets"].entries !== "function") {
		throw new Error("required runtime query API does not expose entries(): at.query.assetRegistry.assets");
	}
	return queries;
}


export async function collectRuntimeConfigurations(payload, at) {
	const queries = requiredRuntimeQueries(at);
	const configurations = [];
	const queryResults = Object.fromEntries(Object.keys(queries).map((storage) => [storage, {
		available: true,
		calls: 0,
		records: 0,
		configurations: 0,
	}]));
	for (const [section, method, component] of CONTRACT_QUERIES) {
		const storage = `${section}.${method}`;
		const addresses = addressValues((await queries[storage]()).toJSON());
		queryResults[storage].calls = 1;
		queryResults[storage].records = addresses.length;
		queryResults[storage].configurations = addresses.length;
		for (const address of addresses) {
			configurations.push(runtimeConfiguration(payload, component, storage, address));
		}
	}
	const entries = await queries["assetRegistry.assets"].entries();
	queryResults["assetRegistry.assets"].calls = 1;
	queryResults["assetRegistry.assets"].records = entries.length;
	queryResults["assetRegistry.assets"].erc20_assets = 0;
	for (const [key, codec] of entries) {
		const assetId = key.args[0];
		const details = codec.toJSON();
		if (!isErc20Asset(details)) continue;
		queryResults["assetRegistry.assets"].erc20_assets += 1;
		const location = (await queries["assetRegistry.assetLocations"](assetId)).toJSON();
		queryResults["assetRegistry.assetLocations"].calls += 1;
		const address = erc20AssetAddress(details, location);
		if (!address) continue;
		queryResults["assetRegistry.assetLocations"].records += 1;
		queryResults["assetRegistry.assetLocations"].configurations += 1;
		configurations.push(runtimeConfiguration(payload, "pallet:asset-registry",
			"assetRegistry.assetLocations", address, { asset_id: assetId.toString(), asset_type: "erc20" }));
	}
	return {
		configurations,
		queryCoverage: {
			required_query_count: Object.keys(queries).length,
			available_query_count: Object.keys(queries).length,
			collected_configuration_count: configurations.length,
			unique_configuration_count: deduplicateConfigurations(configurations).length,
			queries: queryResults,
		},
	};
}


export function outputPayload(payload, configurations, snapshot, inputSha256, versions, queryCoverage) {
	return {
		...payload,
		substrate_snapshot: snapshot,
		chain_context: {
			evm_chain_id: payload.rpc_snapshot.chain_id,
			evm_block_hash: payload.rpc_snapshot.block_hash,
			substrate_genesis_hash: snapshot.genesis_hash,
			substrate_block_hash: snapshot.block_hash,
			substrate_spec_name: snapshot.spec_name,
			block_number: snapshot.block_number,
		},
		runtime_collection_provenance: {
			tool: "collect_runtime_contracts",
			tool_version: COLLECTOR_VERSION,
			node_version: versions.node,
			polkadot_api_version: versions.polkadotApi,
			collector_sha256: versions.collectorSha256,
			input_sha256: inputSha256,
			query_coverage: queryCoverage,
		},
		runtime_configurations: deduplicateConfigurations(configurations),
	};
}


export async function main(argv = process.argv.slice(2)) {
	const parsed = options(argv);
	const input = requiredOption(parsed, "--input");
	const output = requiredOption(parsed, "--output");
	const rpc = requiredOption(parsed, "--rpc");
	const expectedGenesisHash = requiredOption(parsed, "--expected-genesis-hash");
	const expectedSpecName = requiredOption(parsed, "--expected-spec-name");
	const requestedBlock = parsed.get("--block") ?? null;
	const requestedNumberValue = parsed.get("--block-number") ?? null;
	if ((requestedBlock === null) === (requestedNumberValue === null)) {
		throw new Error("exactly one of --block or --block-number is required");
	}
	const requestedNumber = requestedNumberValue === null ? null : Number(requestedNumberValue);
	if (requestedNumberValue !== null && (!Number.isSafeInteger(requestedNumber) || requestedNumber < 0)) {
		throw new Error(`invalid block number: ${requestedNumberValue}`);
	}
	const inputBytes = fs.readFileSync(input);
	const payload = JSON.parse(inputBytes.toString("utf8"));
	validateInputPayload(payload, requestedNumber);
	const { ApiPromise, WsProvider } = await import(
		"../../launch-configs/fork/node_modules/@polkadot/api/index.js"
	);
	const provider = new WsProvider(rpc);
	const api = await ApiPromise.create({ provider, noInitWarn: true });
	try {
		const hash = requestedBlock || (await api.rpc.chain.getBlockHash(requestedNumber)).toHex();
		const header = await api.rpc.chain.getHeader(hash);
		const blockNumber = header.number.toNumber();
		if (blockNumber !== payload.rpc_snapshot.block_number) {
			throw new Error(`Substrate block ${blockNumber} does not match EVM block ${payload.rpc_snapshot.block_number}`);
		}
		const genesisHash = (await api.rpc.chain.getBlockHash(0)).toHex();
		const chain = (await api.rpc.system.chain()).toString();
		const runtimeVersion = await api.rpc.state.getRuntimeVersion(hash);
		const snapshot = {
			rpc,
			chain,
			genesis_hash: genesisHash,
			block_hash: hash,
			block_number: blockNumber,
			parent_hash: header.parentHash.toHex(),
			state_root: header.stateRoot.toHex(),
			extrinsics_root: header.extrinsicsRoot.toHex(),
			spec_name: runtimeVersion.specName.toString(),
			spec_version: runtimeVersion.specVersion.toNumber(),
		};
		validateSubstrateIdentity(snapshot, expectedGenesisHash, expectedSpecName);
		const at = await api.at(hash);
		const { configurations, queryCoverage } = await collectRuntimeConfigurations(payload, at);
		const packagePath = new URL(
			"../../launch-configs/fork/node_modules/@polkadot/api/package.json", import.meta.url
		);
		const polkadotApi = JSON.parse(fs.readFileSync(packagePath, "utf8")).version;
		const result = outputPayload(payload, configurations, snapshot,
			crypto.createHash("sha256").update(inputBytes).digest("hex"),
			{ node: process.version, polkadotApi,
				collectorSha256: crypto.createHash("sha256").update(fs.readFileSync(new URL(import.meta.url))).digest("hex") },
			queryCoverage);
		fs.mkdirSync(path.dirname(path.resolve(output)), { recursive: true });
		fs.writeFileSync(output, `${JSON.stringify(result, null, 2)}\n`);
	} finally {
		await api.disconnect();
	}
}


if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) await main();
