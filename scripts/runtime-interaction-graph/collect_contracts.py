#!/usr/bin/env python3

import argparse
import hashlib
import json
import platform
import re
import subprocess
from pathlib import Path


ADDRESS = re.compile(r"^0x[0-9a-fA-F]{40}$")
HASH_256 = re.compile(r"^0x[0-9a-fA-F]{64}$")
COLLECTOR_VERSION = 3
REQUIRED_RUNTIME_BINDINGS = {
	"gigaHdx.gigaHdxPoolContract",
	"hsm.flashMinter",
	"liquidation.borrowingContract",
}
MASK_64 = (1 << 64) - 1
KECCAK_ROTATIONS = (
	(0, 36, 3, 41, 18),
	(1, 44, 10, 45, 2),
	(62, 6, 43, 15, 61),
	(28, 55, 25, 21, 56),
	(27, 20, 39, 8, 14),
)
KECCAK_ROUND_CONSTANTS = (
	0x0000000000000001, 0x0000000000008082, 0x800000000000808A, 0x8000000080008000,
	0x000000000000808B, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
	0x000000000000008A, 0x0000000000000088, 0x0000000080008009, 0x000000008000000A,
	0x000000008000808B, 0x800000000000008B, 0x8000000000008089, 0x8000000000008003,
	0x8000000000008002, 0x8000000000000080, 0x000000000000800A, 0x800000008000000A,
	0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
)


def sha256_bytes(value: bytes) -> str:
	return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
	return sha256_bytes(path.read_bytes())


def rotate_left(value: int, shift: int) -> int:
	if not shift:
		return value
	return ((value << shift) | (value >> (64 - shift))) & MASK_64


def keccak_f1600(state: list[int]) -> None:
	for constant in KECCAK_ROUND_CONSTANTS:
		columns = [state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20]
			for x in range(5)]
		delta = [columns[(x - 1) % 5] ^ rotate_left(columns[(x + 1) % 5], 1) for x in range(5)]
		for y in range(5):
			for x in range(5):
				state[x + 5 * y] ^= delta[x]
		rotated = [0] * 25
		for y in range(5):
			for x in range(5):
				rotated[y + 5 * ((2 * x + 3 * y) % 5)] = rotate_left(
					state[x + 5 * y], KECCAK_ROTATIONS[x][y])
		for y in range(5):
			for x in range(5):
				state[x + 5 * y] = rotated[x + 5 * y] ^ (
					(~rotated[(x + 1) % 5 + 5 * y]) & rotated[(x + 2) % 5 + 5 * y])
		state[0] ^= constant


def keccak256(value: bytes) -> bytes:
	rate = 136
	padded = bytearray(value)
	padded.append(0x01)
	padded.extend(b"\x00" * ((rate - len(padded) % rate) % rate))
	padded[-1] ^= 0x80
	state = [0] * 25
	for offset in range(0, len(padded), rate):
		block = padded[offset:offset + rate]
		for lane in range(rate // 8):
			state[lane] ^= int.from_bytes(block[lane * 8:(lane + 1) * 8], "little")
		keccak_f1600(state)
	return b"".join(lane.to_bytes(8, "little") for lane in state)[:32]


def canonical_abi_type(parameter: dict) -> str:
	type_name = parameter.get("type")
	if not isinstance(type_name, str) or not type_name:
		raise ValueError(f"ABI parameter has no type: {parameter!r}")
	if not type_name.startswith("tuple"):
		match = re.fullmatch(r"(uint|int)((?:\[[0-9]*\])*)", type_name)
		return f"{match.group(1)}256{match.group(2)}" if match else type_name
	components = parameter.get("components")
	if not isinstance(components, list):
		raise ValueError(f"tuple ABI parameter has no components: {parameter!r}")
	canonical = ",".join(canonical_abi_type(component) for component in components)
	return f"({canonical}){type_name[len('tuple'):]}"


def abi_functions(abi: list[dict]) -> list[dict]:
	functions = {}
	for item in abi:
		if item.get("type") != "function" or not item.get("name"):
			continue
		signature = f"{item['name']}({','.join(canonical_abi_type(value) for value in item.get('inputs', []))})"
		functions[signature] = {"signature": signature, "selector": f"0x{keccak256(signature.encode())[:4].hex()}"}
	return [functions[signature] for signature in sorted(functions)]


def abi_signatures(abi: list[dict]) -> list[str]:
	return [item["signature"] for item in abi_functions(abi)]


def git_repository(path: Path) -> Path | None:
	current = path.resolve()
	for candidate in (current, *current.parents):
		if (candidate / ".git").exists():
			return candidate
	return None


def git_output(repository: Path, *args: str) -> str | None:
	result = subprocess.run(["git", "-C", repository, *args], capture_output=True, text=True, check=False)
	return result.stdout.strip() if result.returncode == 0 else None


def artifact_name(path: Path, repository: Path | None) -> str:
	if repository:
		try:
			return path.resolve().relative_to(repository).as_posix()
		except ValueError:
			pass
	return path.name


def aggregate_inputs(paths: list[Path], repository: Path | None) -> str:
	digest = hashlib.sha256()
	for path in sorted(paths):
		digest.update(artifact_name(path, repository).encode())
		digest.update(b"\0")
		digest.update(bytes.fromhex(sha256_file(path)))
	return digest.hexdigest()


def hardhat_deployments(project: str, root: Path, networks: set[str]) -> tuple[list[dict], list[Path]]:
	contracts = []
	paths = []
	for path in sorted(root.glob("*/*.json")):
		if path.parent.name not in networks or path.name.startswith((".", "_")):
			continue
		paths.append(path)
		try:
			data = json.loads(path.read_text())
		except json.JSONDecodeError as error:
			raise ValueError(f"invalid deployment JSON: {path}") from error
		address = data.get("address")
		if not isinstance(address, str) or not ADDRESS.fullmatch(address):
			continue
		receipt = data.get("receipt") or {}
		functions = abi_functions(data.get("abi", []))
		contracts.append({"project": project, "network": path.parent.name, "name": path.stem,
			"address": address.lower(), "artifact_path": path, "artifact_sha256": sha256_file(path),
			"transaction_hash": data.get("transactionHash") or receipt.get("transactionHash"),
			"block_number": receipt.get("blockNumber"), "abi_functions": functions,
			"abi_signatures": [item["signature"] for item in functions]})
	return contracts, paths


def address_values(value: object, prefix: str = "") -> list[tuple[str, str]]:
	result = []
	if isinstance(value, dict):
		for key, child in value.items():
			result.extend(address_values(child, f"{prefix}.{key}" if prefix else key))
	elif isinstance(value, list):
		for index, child in enumerate(value):
			result.extend(address_values(child, f"{prefix}.{index}" if prefix else str(index)))
	elif isinstance(value, str) and ADDRESS.fullmatch(value):
		result.append((prefix, value.lower()))
	return result


def whm_field_name(field: str) -> str:
	return field.rsplit(".", 1)[-1].replace("_", "").lower()


def whm_deployment_role(step: str, field: str) -> str | None:
	if not re.search(r"(?:^|[-_])deploy(?:$|[-_])", step.lower()):
		return None
	return {
		"address": "contract",
		"contract": "contract",
		"contractaddress": "contract",
		"implementation": "implementation",
		"implementationaddress": "implementation",
		"impladdress": "implementation",
		"proxy": "proxy",
		"proxyaddress": "proxy",
	}.get(whm_field_name(field))


def whm_reference_role(field: str, address: str) -> str:
	name = whm_field_name(field)
	if address == "0x" + "0" * 40:
		return "null-address"
	if name in {"asset", "sourceasset", "destasset", "wrappednative"}:
		return "asset"
	if "oracle" in name:
		return "oracle"
	if "handler" in name:
		return "handler"
	if name in {"bridge", "bridgeaddress", "mdah160"}:
		return "bridge"
	if "transactor" in name:
		return "transactor"
	if "receiver" in name:
		return "receiver"
	if any(value in name for value in ("owner", "operator", "relayer")):
		return "authorization-account"
	if name in {"contract", "contractaddress", "implementation", "implementationaddress", "impladdress",
		"proxy", "proxyaddress"}:
		return "contract-reference"
	return "address-reference"


def whm_deployments(project: str, root: Path,
	networks: set[str]) -> tuple[list[dict], list[dict], list[Path]]:
	contracts = []
	references = []
	paths = []
	for path in sorted(root.glob("*.json")):
		if path.stem not in networks:
			continue
		paths.append(path)
		try:
			data = json.loads(path.read_text())
		except json.JSONDecodeError as error:
			raise ValueError(f"invalid deployment JSON: {path}") from error
		for step in data.get("steps", []):
			if step.get("status") != "completed":
				continue
			for field, address in address_values(step.get("output", {})):
				role = whm_deployment_role(step["name"], field)
				entry = {"project": project, "network": path.stem, "address": address,
					"artifact_path": path, "artifact_sha256": sha256_file(path),
					"migration_step": step["name"], "field": field}
				if role:
					contracts.append({**entry, "name": f"{step['name']}:{field}", "deployment_role": role,
						"abi_functions": [], "abi_signatures": []})
				else:
					references.append({**entry, "role": whm_reference_role(field, address)})
	return contracts, references, paths


def load_descriptor(path: Path) -> dict:
	try:
		payload = json.loads(path.read_text())
	except json.JSONDecodeError as error:
		raise ValueError(f"invalid deployment descriptor JSON: {path}") from error
	if payload.get("schema_version") != 2 or not isinstance(payload.get("sources"), list):
		raise ValueError("deployment descriptor must use schema_version 2 and define sources")
	projects = set()
	for source in payload["sources"]:
		required = {"project", "kind", "root", "networks"}
		if not isinstance(source, dict) or not required <= source.keys() or source["kind"] not in {"hardhat", "whm"}:
			raise ValueError(f"invalid deployment source descriptor: {source!r}")
		if (not isinstance(source["project"], str) or not source["project"] or
			not isinstance(source["root"], str) or not source["root"] or
			not isinstance(source["networks"], list) or not source["networks"] or
			any(not isinstance(network, str) or not network for network in source["networks"]) or
			len(source["networks"]) != len(set(source["networks"])) or source["project"] in projects):
			raise ValueError(f"duplicate project or empty network set: {source['project']}")
		projects.add(source["project"])
	if not isinstance(payload.get("chains"), list) or not payload["chains"]:
		raise ValueError("deployment descriptor must define at least one chain")
	chain_ids = set()
	for chain in payload["chains"]:
		minimums = chain.get("runtime_configuration_minimums") if isinstance(chain, dict) else None
		binding_minimums = minimums.get("required_bindings") if isinstance(minimums, dict) else None
		if (not isinstance(chain, dict) or not isinstance(chain.get("id"), str) or not chain["id"] or
			not isinstance(chain.get("evm_chain_id"), int) or isinstance(chain["evm_chain_id"], bool) or
			chain["evm_chain_id"] < 1 or not isinstance(chain.get("substrate_genesis_hash"), str) or
			not HASH_256.fullmatch(chain["substrate_genesis_hash"]) or
			not isinstance(chain.get("substrate_spec_name"), str) or not chain["substrate_spec_name"] or
			not isinstance(minimums, dict) or set(minimums) != {"total", "asset_registry_erc20", "required_bindings"} or
			any(not isinstance(minimums.get(field), int) or isinstance(minimums.get(field), bool)
				or minimums[field] < 1 for field in ("total", "asset_registry_erc20")) or
			not isinstance(binding_minimums, dict) or set(binding_minimums) != REQUIRED_RUNTIME_BINDINGS or
			any(not isinstance(value, int) or isinstance(value, bool) or value < 1
				for value in binding_minimums.values()) or
			not isinstance(chain.get("deployment_networks"), list) or
			not chain["deployment_networks"] or any(not isinstance(network, str) or not network
				for network in chain["deployment_networks"]) or
			len(chain["deployment_networks"]) != len(set(chain["deployment_networks"])) or chain["id"] in chain_ids):
			raise ValueError(f"invalid or duplicate deployment chain descriptor: {chain!r}")
		chain_ids.add(chain["id"])
	return payload


def collect(descriptor_path: Path) -> dict:
	descriptor = load_descriptor(descriptor_path)
	contracts = []
	references = []
	sources = []
	for source in descriptor["sources"]:
		root = (descriptor_path.parent / source["root"]).resolve()
		if not root.is_dir():
			raise FileNotFoundError(f"required deployment source is missing: {root}")
		networks = set(source["networks"])
		if source["kind"] == "hardhat":
			items, inputs = hardhat_deployments(source["project"], root, networks)
			source_references = []
		else:
			items, source_references, inputs = whm_deployments(source["project"], root, networks)
		if not inputs or not items:
			raise ValueError(f"required deployment source produced no contracts: {source['project']}")
		repository = git_repository(root)
		for item in items:
			item["artifact"] = artifact_name(item.pop("artifact_path"), repository)
		for reference in source_references:
			reference["artifact"] = artifact_name(reference.pop("artifact_path"), repository)
		contracts.extend(items)
		references.extend(source_references)
		sources.append({"project": source["project"], "kind": source["kind"],
			"configured_root": source["root"], "networks": sorted(networks),
			"git_commit": git_output(repository, "rev-parse", "HEAD") if repository else None,
			"git_dirty": bool(git_output(repository, "status", "--porcelain")) if repository else None,
			"input_count": len(inputs), "input_sha256": aggregate_inputs(inputs, repository),
			"contract_count": len(items), "address_reference_count": len(source_references)})
	return {"schema_version": 2,
		"collection_provenance": {"tool": "collect_contracts", "tool_version": COLLECTOR_VERSION,
			"python_version": platform.python_version(), "collector_sha256": sha256_file(Path(__file__)),
			"descriptor": descriptor_path.name,
			"descriptor_sha256": sha256_file(descriptor_path), "sources": sources},
		"chains": descriptor["chains"], "contracts": sorted(contracts,
			key=lambda item: (item["project"], item["network"], item["address"], item["name"])),
		"address_references": sorted(references, key=lambda item: (
			item["project"], item["network"], item["address"], item["migration_step"], item["field"]))}


def main() -> None:
	parser = argparse.ArgumentParser()
	parser.add_argument("--descriptor", type=Path, required=True)
	parser.add_argument("--output", type=Path, required=True)
	args = parser.parse_args()
	payload = collect(args.descriptor.resolve())
	args.output.parent.mkdir(parents=True, exist_ok=True)
	args.output.write_text(json.dumps(payload, indent=2) + "\n")


if __name__ == "__main__":
	main()
