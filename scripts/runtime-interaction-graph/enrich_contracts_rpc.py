#!/usr/bin/env python3

import argparse
import hashlib
import json
import platform
import urllib.request
from pathlib import Path


EIP1967_IMPLEMENTATION = "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"
ENRICHER_VERSION = 3


def rpc(url: str, method: str, params: list) -> object:
	payload = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
	request = urllib.request.Request(url, payload, {"Content-Type": "application/json"})
	with urllib.request.urlopen(request, timeout=30) as response:
		result = json.loads(response.read())
	if "error" in result:
		raise RuntimeError(result["error"])
	return result["result"]


def rpc_batch(url: str, calls: list[tuple[str, list]]) -> list[object]:
	payload = [{"jsonrpc": "2.0", "id": index, "method": method, "params": params}
		for index, (method, params) in enumerate(calls)]
	request = urllib.request.Request(url, json.dumps(payload).encode(), {"Content-Type": "application/json"})
	with urllib.request.urlopen(request, timeout=120) as response:
		results = json.loads(response.read())
	by_id = {item["id"]: item for item in results}
	values = []
	for index in range(len(calls)):
		item = by_id.get(index)
		if not item or "error" in item or "result" not in item:
			raise RuntimeError(f"invalid batch RPC response for call {index}: {item!r}")
		values.append(item["result"])
	return values


def block_tag(value: str) -> str:
	if value in {"latest", "safe", "finalized"} or value.startswith("0x"):
		return value
	return hex(int(value))


def validate_collection(payload: dict) -> None:
	if payload.get("schema_version") != 2:
		raise ValueError("deployment collection must use schema_version 2")
	provenance = payload.get("collection_provenance")
	if not isinstance(provenance, dict) or not provenance.get("descriptor_sha256") or not provenance.get("sources"):
		raise ValueError("deployment collection has no verifiable source provenance")
	if not isinstance(payload.get("contracts"), list) or not payload["contracts"]:
		raise ValueError("deployment collection has no contracts")
	if any(not source.get("input_sha256") for source in provenance["sources"]):
		raise ValueError("deployment collection has an unhashed source inventory")
	if any(not contract.get("artifact_sha256") for contract in payload["contracts"]):
		raise ValueError("deployment collection has an unhashed contract artifact")
	references = payload.get("address_references", [])
	if not isinstance(references, list) or any(
		not isinstance(reference, dict) or not reference.get("artifact_sha256") or
		not reference.get("role") or not reference.get("field")
		for reference in references
	):
		raise ValueError("deployment collection has an invalid address reference inventory")


def enrich(payload: dict, url: str, block: str, networks: set[str], expected_chain_id: int,
	input_sha256: str, rpc_call=rpc, rpc_batch_call=rpc_batch) -> dict:
	validate_collection(payload)
	if not networks:
		raise ValueError("at least one deployment network must be selected")
	selected = [contract for contract in payload["contracts"] if contract["network"] in networks]
	missing_networks = networks - {contract["network"] for contract in selected}
	if missing_networks:
		raise ValueError(f"selected deployment networks have no contracts: {sorted(missing_networks)}")
	chain_id = int(rpc_call(url, "eth_chainId", []), 16)
	if chain_id != expected_chain_id:
		raise ValueError(f"RPC chain id {chain_id} does not match expected chain id {expected_chain_id}")
	requested_block = block_tag(block)
	block_number = rpc_call(url, "eth_blockNumber", []) if requested_block == "latest" else requested_block
	block_data = rpc_call(url, "eth_getBlockByNumber", [block_number, False])
	if not block_data or not block_data.get("hash"):
		raise RuntimeError(f"RPC returned no block for {block_number}")
	resolved_number = int(block_data["number"], 16)
	if requested_block not in {"latest", "safe", "finalized"} and int(requested_block, 16) != resolved_number:
		raise RuntimeError(f"RPC returned block {resolved_number} for requested block {requested_block}")
	known = {contract["address"].lower() for contract in selected}
	observations = []
	unique = {(contract["project"], contract["network"], contract["address"].lower()): contract
		for contract in selected}
	calls = []
	for _, _, address in sorted(unique):
		calls.extend([("eth_getCode", [address, block_data["number"]]),
			("eth_getStorageAt", [address, EIP1967_IMPLEMENTATION, block_data["number"]])])
	results = iter(rpc_batch_call(url, calls))
	for project, network, address in sorted(unique):
		contract = unique[(project, network, address)]
		code = next(results)
		implementation_word = next(results)
		implementation = "0x" + implementation_word[-40:] if int(implementation_word, 16) else None
		embedded = sorted(candidate for candidate in known if candidate != address and candidate[2:] in code.lower())
		has_code = code != "0x"
		classification = "proxy" if implementation else ("deployed" if has_code else
			("no-code-implementation-artifact" if "implementation" in contract["name"].lower()
			else "stale-or-undeployed-artifact"))
		observations.append({"project": project, "network": network, "chain_id": chain_id, "address": address,
			"chain_address_id": f"eip155:{chain_id}:{address}", "has_code": has_code,
			"classification": classification,
			"bytecode_sha256": hashlib.sha256(bytes.fromhex(code[2:])).hexdigest(),
			"bytecode_size": (len(code) - 2) // 2, "implementation": implementation,
			"implementation_chain_address_id": f"eip155:{chain_id}:{implementation}" if implementation else None,
			"embedded_addresses": embedded})
	return {**payload, "rpc_snapshot": {"url": url, "chain_id": chain_id,
		"requested_block": block, "block_number": resolved_number, "block_hash": block_data["hash"],
		"parent_hash": block_data.get("parentHash"), "state_root": block_data.get("stateRoot")},
		"enrichment_provenance": {"tool": "enrich_contracts_rpc", "tool_version": ENRICHER_VERSION,
			"python_version": platform.python_version(),
			"collector_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
			"input_sha256": input_sha256,
			"selected_networks": sorted(networks)}, "observations": observations}


def main() -> None:
	parser = argparse.ArgumentParser()
	parser.add_argument("--input", type=Path, required=True)
	parser.add_argument("--output", type=Path, required=True)
	parser.add_argument("--rpc", required=True)
	parser.add_argument("--block", required=True)
	parser.add_argument("--expected-chain-id", type=int, required=True)
	parser.add_argument("--network", action="append", required=True)
	args = parser.parse_args()
	input_bytes = args.input.read_bytes()
	payload = enrich(json.loads(input_bytes), args.rpc, args.block, set(args.network), args.expected_chain_id,
		hashlib.sha256(input_bytes).hexdigest())
	args.output.parent.mkdir(parents=True, exist_ok=True)
	args.output.write_text(json.dumps(payload, indent=2) + "\n")


if __name__ == "__main__":
	main()
