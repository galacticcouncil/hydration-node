#!/usr/bin/env python3

import argparse
import hashlib
import json
import platform
import re
import subprocess
import sys
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parent))

import runtime_interaction_graph as runtime_graph


BUILD_VERSION = 4
PINNED_BLOCK = re.compile(r"^(?:0x[0-9a-fA-F]+|[0-9]+)$")
HASH_256 = re.compile(r"^0x[0-9a-fA-F]{64}$")
GRAPH_GENERATOR_INPUTS = (
	"runtime_interaction_graph.py",
	"graph_explorer.js",
	"graph_explorer.css",
	"mir_parser.py",
	"runtime_inventory.py",
	"semantic_inventory.py",
	"semantic-inventory.json",
	"analysis_provenance.py",
	"collect_contracts.py",
	"collect_mir.py",
	"collect_rapx.py",
	"historical-interactions.json",
)
REQUIRED_RUNTIME_BINDINGS = {
	"gigaHdx.gigaHdxPoolContract": "pallet:gigahdx",
	"hsm.flashMinter": "pallet:hsm",
	"liquidation.borrowingContract": "pallet:liquidation",
}


def sha256_file(path: Path) -> str:
	return hashlib.sha256(path.read_bytes()).hexdigest()


def source_fingerprint(root: Path) -> dict:
	return runtime_graph.analysis_provenance.tree_fingerprint(root)


def load_chain(descriptor: Path, chain_id: str) -> dict:
	payload = json.loads(descriptor.read_text())
	if payload.get("schema_version") != 2:
		raise ValueError("deployment descriptor must use schema_version 2")
	matches = [chain for chain in payload.get("chains", []) if chain.get("id") == chain_id]
	if len(matches) != 1:
		raise ValueError(f"deployment descriptor has no unique chain named {chain_id}")
	chain = matches[0]
	minimums = chain.get("runtime_configuration_minimums")
	binding_minimums = minimums.get("required_bindings") if isinstance(minimums, dict) else None
	if (not isinstance(chain.get("evm_chain_id"), int) or isinstance(chain.get("evm_chain_id"), bool) or
		chain["evm_chain_id"] < 1 or not chain.get("deployment_networks") or
		not isinstance(chain.get("substrate_genesis_hash"), str) or
		not HASH_256.fullmatch(chain["substrate_genesis_hash"]) or
		not isinstance(chain.get("substrate_spec_name"), str) or not chain["substrate_spec_name"] or
		not isinstance(minimums, dict) or set(minimums) != {"total", "asset_registry_erc20", "required_bindings"} or
		any(not isinstance(minimums.get(field), int) or isinstance(minimums.get(field), bool)
			or minimums[field] < 1 for field in ("total", "asset_registry_erc20")) or
		not isinstance(binding_minimums, dict) or set(binding_minimums) != set(REQUIRED_RUNTIME_BINDINGS) or
		any(not isinstance(value, int) or isinstance(value, bool) or value < 1
			for value in binding_minimums.values())):
		raise ValueError(f"invalid chain descriptor: {chain!r}")
	return chain


def validate_runtime_configuration_coverage(payload: dict, minimums: dict) -> dict:
	configurations = payload.get("runtime_configurations")
	query_coverage = payload.get("runtime_collection_provenance", {}).get("query_coverage", {})
	if not isinstance(configurations, list) or query_coverage.get("required_query_count") != 5 \
		or query_coverage.get("available_query_count") != query_coverage.get("required_query_count"):
		raise ValueError("runtime contract collection has incomplete required-query coverage")
	registry_erc20 = sum(configuration.get("component") == "pallet:asset-registry"
		and configuration.get("storage") == "assetRegistry.assetLocations"
		and configuration.get("asset_type") == "erc20" for configuration in configurations)
	bindings = {storage: sum(configuration.get("component") == component
		and configuration.get("storage") == storage for configuration in configurations)
		for storage, component in REQUIRED_RUNTIME_BINDINGS.items()}
	coverage = {"total": len(configurations), "asset_registry_erc20": registry_erc20,
		"required_bindings": bindings, "required_queries": query_coverage["required_query_count"]}
	for field in ("total", "asset_registry_erc20"):
		if coverage[field] < minimums[field]:
			raise ValueError(
				f"runtime configuration coverage {field}={coverage[field]} is below minimum {minimums[field]}"
			)
	for storage, minimum in minimums["required_bindings"].items():
		if bindings[storage] < minimum:
			raise ValueError(
				f"runtime configuration coverage {storage}={bindings[storage]} is below minimum {minimum}"
			)
	return coverage


def logical_path(path: Path, root: Path | None = None) -> str:
	if root is not None:
		try:
			return path.resolve().relative_to(root.resolve()).as_posix()
		except ValueError:
			pass
	return path.name


def validate_semantic_manifest(path: Path, expected_tool: str, expected_source_inputs: dict | None = None,
	root: Path | None = None) -> dict:
	if not path.is_file():
		raise FileNotFoundError(f"semantic manifest does not exist: {path}")
	payload = json.loads(path.read_text())
	runtime_graph.validate_semantic_manifest(payload, path, expected_tool, root)
	provenance = payload.get("provenance", {})
	if expected_source_inputs is not None and provenance["source_inputs"] != expected_source_inputs:
		raise ValueError(f"semantic manifest was produced from stale or different source inputs: {path}")
	artifacts = []
	if expected_tool == "rustc-mir":
		artifacts = [(entry.get("artifact"), entry.get("artifact_sha256"))
			for entry in payload.get("packages", []) if entry.get("status") == "ok"]
	else:
		artifacts = [(analysis.get("path"), analysis.get("artifact_sha256"))
			for entry in payload.get("packages", []) for analysis in entry.get("analyses", {}).values()
			if analysis.get("status") == "ok"]
	return {"path": logical_path(path, root), "sha256": sha256_file(path), "tool": expected_tool,
		"toolchain": payload.get("toolchain"), "source_inputs": provenance["source_inputs"],
		"collector_sha256": provenance["collector_sha256"], "tool_inputs": provenance.get("tool_inputs"),
		"artifacts": [{"path": artifact, "sha256": expected_sha256}
			for artifact, expected_sha256 in artifacts], "packages": len(payload.get("packages", [])),
		"validated": True}


def git_state(root: Path) -> dict:
	commit = subprocess.run(["git", "-C", root, "rev-parse", "HEAD"], capture_output=True, text=True, check=True)
	status = subprocess.run(["git", "-C", root, "status", "--porcelain"], capture_output=True, text=True, check=True)
	return {"commit": commit.stdout.strip(), "dirty": bool(status.stdout.strip())}


def graph_command(scripts: Path, runtime: Path, output: Path, rapx_manifest: Path | None,
	rustc_mir_manifest: Path | None) -> list[object]:
	thresholds = scripts / ("coverage-thresholds-full.json" if rustc_mir_manifest else "coverage-thresholds.json")
	command: list[object] = ["python3", scripts / "runtime_interaction_graph.py", "--contracts-manifest", runtime,
		"--output", output, "--coverage-thresholds", thresholds]
	if rapx_manifest:
		command.extend(["--rapx-manifest", rapx_manifest])
	if rustc_mir_manifest:
		command.extend(["--rustc-mir-manifest", rustc_mir_manifest])
	return command


def run(command: list[object], root: Path) -> None:
	subprocess.run([str(value) for value in command], cwd=root, check=True)


def output_hashes(output: Path) -> dict[str, str]:
	return {path.relative_to(output).as_posix(): sha256_file(path)
		for path in sorted(output.rglob("*"))
		if path.is_file() and path.name != "evm-build-provenance.json"}


def graph_generator_fingerprint(scripts: Path, rustc_mir_manifest: Path | None) -> dict:
	thresholds = "coverage-thresholds-full.json" if rustc_mir_manifest else "coverage-thresholds.json"
	paths = [scripts / relative for relative in (*GRAPH_GENERATOR_INPUTS, thresholds)]
	return runtime_graph.analysis_provenance.tool_input_fingerprint(paths)


def prepare_output(output: Path) -> None:
	if output.exists() and (not output.is_dir() or any(output.iterdir())):
		raise ValueError(f"output directory must be empty: {output}")
	output.mkdir(parents=True, exist_ok=True)


def main() -> None:
	parser = argparse.ArgumentParser()
	parser.add_argument("--descriptor", type=Path, required=True)
	parser.add_argument("--chain", required=True)
	parser.add_argument("--evm-rpc", required=True)
	parser.add_argument("--substrate-rpc", required=True)
	parser.add_argument("--block", required=True, help="Pinned EVM block number in decimal or 0x-prefixed form")
	parser.add_argument("--output", type=Path, required=True)
	parser.add_argument("--rapx-manifest", type=Path)
	parser.add_argument("--rustc-mir-manifest", type=Path)
	args = parser.parse_args()
	if not PINNED_BLOCK.fullmatch(args.block):
		parser.error("--block must be a pinned decimal or 0x-prefixed block number")
	root = Path(__file__).resolve().parents[2]
	scripts = Path(__file__).parent
	descriptor = args.descriptor.resolve()
	chain = load_chain(descriptor, args.chain)
	output = args.output.resolve()
	prepare_output(output)
	contracts = output / "contracts.json"
	onchain = output / "contracts-onchain.json"
	runtime = output / "contracts-runtime.json"
	run(["python3", scripts / "collect_contracts.py", "--descriptor", descriptor, "--output", contracts], root)
	enrich_command: list[object] = ["python3", scripts / "enrich_contracts_rpc.py", "--input", contracts,
		"--output", onchain, "--rpc", args.evm_rpc, "--block", args.block,
		"--expected-chain-id", chain["evm_chain_id"]]
	for network in chain["deployment_networks"]:
		enrich_command.extend(["--network", network])
	run(enrich_command, root)
	onchain_payload = json.loads(onchain.read_text())
	block_number = onchain_payload["rpc_snapshot"]["block_number"]
	run(["node", scripts / "collect_runtime_contracts.mjs", "--input", onchain, "--output", runtime,
		"--rpc", args.substrate_rpc, "--block-number", block_number,
		"--expected-genesis-hash", chain["substrate_genesis_hash"],
		"--expected-spec-name", chain["substrate_spec_name"]], root)
	runtime_payload = json.loads(runtime.read_text())
	chain_context = runtime_payload.get("chain_context", {})
	if chain_context.get("block_number") != block_number:
		raise RuntimeError("runtime collection did not preserve the pinned block number")
	if chain_context.get("substrate_genesis_hash", "").lower() != chain["substrate_genesis_hash"].lower():
		raise RuntimeError("runtime collection did not preserve the expected Substrate genesis hash")
	if chain_context.get("substrate_spec_name") != chain["substrate_spec_name"]:
		raise RuntimeError("runtime collection did not preserve the expected Substrate spec name")
	runtime_configuration_coverage = validate_runtime_configuration_coverage(
		runtime_payload, chain["runtime_configuration_minimums"])
	semantic_inputs = []
	current_source_inputs = source_fingerprint(root)
	if args.rapx_manifest:
		semantic_inputs.append(validate_semantic_manifest(args.rapx_manifest, "rapx", current_source_inputs, root))
	if args.rustc_mir_manifest:
		semantic_inputs.append(validate_semantic_manifest(
			args.rustc_mir_manifest, "rustc-mir", current_source_inputs, root))
	run(graph_command(scripts, runtime, output, args.rapx_manifest, args.rustc_mir_manifest), root)
	provenance = {
		"schema_version": 2,
		"tool": "build_evm_graph",
		"tool_version": BUILD_VERSION,
		"python_version": platform.python_version(),
		"collector_sha256": sha256_file(Path(__file__)),
		"node_version": subprocess.run(["node", "--version"], capture_output=True, text=True, check=True).stdout.strip(),
		"repository": git_state(root),
		"source_inputs": current_source_inputs,
		"descriptor": {"path": descriptor.name, "sha256": sha256_file(descriptor), "chain": args.chain},
		"graph_generator": graph_generator_fingerprint(scripts, args.rustc_mir_manifest),
		"semantic_inputs": semantic_inputs,
		"snapshot": runtime_payload["chain_context"],
		"runtime_configuration_coverage": runtime_configuration_coverage,
		"outputs": output_hashes(output),
	}
	(output / "evm-build-provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")


if __name__ == "__main__":
	main()
