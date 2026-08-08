#!/usr/bin/env python3

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path
from typing import Any


SCHEMA_VERSION = 1
ID_PATTERN = re.compile(r"^[a-z][a-z0-9-]*(?::[a-z0-9][a-z0-9-]*)+$")
ALLOWED_NODE_KINDS = frozenset({
	"component",
	"configuration",
	"guard",
	"invariant",
	"ledger",
	"operation",
	"pool-state",
	"router",
})
ALLOWED_DOMAINS = frozenset({"asset-routing", "circuit-breaker", "omnipool", "stableswap", "xyk"})
ALLOWED_EDGE_KINDS = frozenset({
	"backs",
	"burns",
	"configured-by",
	"derives-from",
	"enforces",
	"guards",
	"invokes",
	"locks",
	"mints",
	"must-equal",
	"owns",
	"reads",
	"routes-to",
	"tracks",
	"transfers-from",
	"transfers-to",
	"updates",
	"writes",
})
ALLOWED_ENFORCEMENT = frozenset({
	"calculation",
	"configuration",
	"coupled-update",
	"observation",
	"runtime",
	"transactional",
	"try-runtime",
})


class InventoryError(ValueError):
	pass


def _require(condition: bool, message: str) -> None:
	if not condition:
		raise InventoryError(message)


def _require_object(value: Any, context: str) -> dict[str, Any]:
	_require(isinstance(value, dict), f"{context} must be an object")
	return value


def _require_string(value: Any, context: str) -> str:
	_require(isinstance(value, str) and bool(value.strip()), f"{context} must be a non-empty string")
	return value


def _require_keys(value: dict[str, Any], required: set[str], context: str) -> None:
	missing = required - value.keys()
	unknown = value.keys() - required
	_require(not missing, f"{context} is missing keys: {', '.join(sorted(missing))}")
	_require(not unknown, f"{context} has unknown keys: {', '.join(sorted(unknown))}")


def _source_evidence(
	evidence: Any,
	root: Path,
	context: str,
	cache: dict[Path, tuple[str, str]],
) -> list[dict[str, Any]]:
	_require(isinstance(evidence, list) and bool(evidence), f"{context}.evidence must be a non-empty array")
	normalized = []
	for index, raw_item in enumerate(evidence):
		item_context = f"{context}.evidence[{index}]"
		item = _require_object(raw_item, item_context)
		_require_keys(item, {"file", "symbol"}, item_context)
		relative_text = _require_string(item["file"], f"{item_context}.file")
		symbol = _require_string(item["symbol"], f"{item_context}.symbol")
		relative = Path(relative_text)
		_require(not relative.is_absolute(), f"{item_context}.file must be relative to the repository root")
		_require(".." not in relative.parts, f"{item_context}.file cannot traverse outside the repository root")
		path = (root / relative).resolve()
		try:
			path.relative_to(root)
		except ValueError as error:
			raise InventoryError(f"{item_context}.file resolves outside the repository root") from error
		_require(path.suffix == ".rs", f"{item_context}.file must reference a Rust source file")
		_require(path.is_file(), f"{item_context}.file does not exist: {relative_text}")
		if path not in cache:
			contents = path.read_text(encoding="utf-8")
			cache[path] = (contents, hashlib.sha256(contents.encode()).hexdigest())
		contents, source_sha256 = cache[path]
		offset = contents.find(symbol)
		_require(offset >= 0, f"{item_context}.symbol was not found in {relative_text}: {symbol}")
		normalized.append({
			"file": relative.as_posix(),
			"symbol": symbol,
			"line": contents.count("\n", 0, offset) + 1,
			"source_sha256": source_sha256,
		})
	return normalized


def validate_inventory(payload: Any, root: Path) -> dict[str, Any]:
	root = root.resolve()
	_require(root.is_dir(), f"repository root does not exist: {root}")
	document = _require_object(payload, "inventory")
	_require_keys(document, {"schema_version", "nodes", "edges"}, "inventory")
	_require(document["schema_version"] == SCHEMA_VERSION, f"schema_version must be {SCHEMA_VERSION}")
	_require(isinstance(document["nodes"], list), "inventory.nodes must be an array")
	_require(isinstance(document["edges"], list), "inventory.edges must be an array")

	cache: dict[Path, tuple[str, str]] = {}
	nodes = []
	node_ids = set()
	for index, raw_node in enumerate(document["nodes"]):
		context = f"inventory.nodes[{index}]"
		node = _require_object(raw_node, context)
		_require_keys(node, {"id", "kind", "domain", "label", "description", "evidence"}, context)
		node_id = _require_string(node["id"], f"{context}.id")
		_require(bool(ID_PATTERN.fullmatch(node_id)), f"{context}.id has an invalid format: {node_id}")
		_require(node_id not in node_ids, f"duplicate node id: {node_id}")
		node_ids.add(node_id)
		kind = _require_string(node["kind"], f"{context}.kind")
		domain = _require_string(node["domain"], f"{context}.domain")
		_require(kind in ALLOWED_NODE_KINDS, f"{context}.kind is unsupported: {kind}")
		_require(domain in ALLOWED_DOMAINS, f"{context}.domain is unsupported: {domain}")
		nodes.append({
			"id": node_id,
			"kind": kind,
			"domain": domain,
			"label": _require_string(node["label"], f"{context}.label"),
			"description": _require_string(node["description"], f"{context}.description"),
			"semantic_source": "explicit-inventory",
			"evidence": _source_evidence(node["evidence"], root, context, cache),
		})

	edges = []
	edge_ids = set()
	for index, raw_edge in enumerate(document["edges"]):
		context = f"inventory.edges[{index}]"
		edge = _require_object(raw_edge, context)
		_require_keys(edge, {"source", "target", "kind", "semantics", "enforcement", "evidence"}, context)
		source = _require_string(edge["source"], f"{context}.source")
		target = _require_string(edge["target"], f"{context}.target")
		kind = _require_string(edge["kind"], f"{context}.kind")
		enforcement = _require_string(edge["enforcement"], f"{context}.enforcement")
		_require(source in node_ids, f"{context}.source references an unknown node: {source}")
		_require(target in node_ids, f"{context}.target references an unknown node: {target}")
		_require(kind in ALLOWED_EDGE_KINDS, f"{context}.kind is unsupported: {kind}")
		_require(enforcement in ALLOWED_ENFORCEMENT, f"{context}.enforcement is unsupported: {enforcement}")
		edge_id = f"{source}|{kind}|{target}"
		_require(edge_id not in edge_ids, f"duplicate semantic edge: {edge_id}")
		edge_ids.add(edge_id)
		edges.append({
			"id": edge_id,
			"source": source,
			"target": target,
			"kind": kind,
			"semantics": _require_string(edge["semantics"], f"{context}.semantics"),
			"enforcement": enforcement,
			"semantic_source": "explicit-inventory",
			"evidence": _source_evidence(edge["evidence"], root, context, cache),
		})

	nodes.sort(key=lambda node: node["id"])
	edges.sort(key=lambda edge: edge["id"])
	evidence_files = sorted({item["file"] for item in nodes + edges for item in item["evidence"]})
	return {
		"schema_version": SCHEMA_VERSION,
		"nodes": nodes,
		"edges": edges,
		"coverage": {
			"node_count": len(nodes),
			"edge_count": len(edges),
			"domains": sorted({node["domain"] for node in nodes}),
			"node_kinds": sorted({node["kind"] for node in nodes}),
			"edge_kinds": sorted({edge["kind"] for edge in edges}),
			"evidence_files": evidence_files,
			"evidence_file_count": len(evidence_files),
		},
	}


def load_inventory(root: Path, inventory_path: Path | None = None) -> dict[str, Any]:
	root_inventory = root / "scripts/runtime-interaction-graph/semantic-inventory.json"
	path = inventory_path or root_inventory
	payload = json.loads(path.read_text(encoding="utf-8"))
	return validate_inventory(payload, root)


def main() -> None:
	default_root = Path(__file__).resolve().parents[2]
	parser = argparse.ArgumentParser(description="Validate and normalize the runtime semantic inventory")
	parser.add_argument("--root", type=Path, default=default_root)
	parser.add_argument("--inventory", type=Path, default=Path(__file__).with_name("semantic-inventory.json"))
	parser.add_argument("--output", type=Path)
	args = parser.parse_args()
	try:
		result = load_inventory(args.root, args.inventory)
	except (InventoryError, json.JSONDecodeError, OSError) as error:
		parser.error(str(error))
	encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
	if args.output:
		args.output.parent.mkdir(parents=True, exist_ok=True)
		args.output.write_text(encoded, encoding="utf-8")
	else:
		print(encoded, end="")


if __name__ == "__main__":
	main()
