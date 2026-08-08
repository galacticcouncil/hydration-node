#!/usr/bin/env python3

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from collections import Counter, defaultdict, deque
from pathlib import Path


SCHEMA_VERSION = 1
TOOL = "runtime-interaction-graph-query"
DEFAULT_MAX_RECORDS = 50
DEFAULT_MAX_TOKENS = 4_000
MAX_QUERY_LENGTH = 512
ACTIVITY_POLICY = (
	"exclude explicit inactive (including inherited owner/function false); "
	"retain missing as unclassified"
)


class QueryFailure(ValueError):
	def __init__(self, code: str, message: str) -> None:
		super().__init__(message)
		self.code = code


def json_key(value: object) -> str:
	return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def load_json(path: Path, expected: type, label: str) -> object:
	try:
		value = json.loads(path.read_text())
	except (OSError, json.JSONDecodeError) as error:
		raise QueryFailure(f"invalid-{label}", f"cannot load {label} from {path}: {error}") from error
	if not isinstance(value, expected):
		raise QueryFailure(f"invalid-{label}", f"{label} must contain a JSON {expected.__name__}")
	return value


class GraphIndex:
	def __init__(self, payload: dict) -> None:
		if payload.get("schema_version") != 2:
			raise QueryFailure("invalid-graph", "graph must use schema_version 2")
		nodes = payload.get("nodes")
		edges = payload.get("edges")
		if not isinstance(nodes, list) or not isinstance(edges, list):
			raise QueryFailure("invalid-graph", "graph must contain node and edge arrays")
		self.nodes: dict[str, dict] = {}
		for node in nodes:
			if not isinstance(node, dict) or not isinstance(node.get("id"), str) or not node["id"]:
				raise QueryFailure("invalid-graph", "every graph node must have a non-empty string id")
			if node["id"] in self.nodes:
				raise QueryFailure("invalid-graph", f"duplicate graph node: {node['id']}")
			self.nodes[node["id"]] = node
		self.edges = []
		for edge in edges:
			if not isinstance(edge, dict) or any(not isinstance(edge.get(field), str) or not edge[field]
				for field in ("source", "target", "kind")):
				raise QueryFailure("invalid-graph", "every graph edge must have source, target, and kind strings")
			if edge["source"] not in self.nodes or edge["target"] not in self.nodes:
				raise QueryFailure("invalid-graph",
					f"dangling graph edge: {edge['source']} -> {edge['target']}")
			self.edges.append(edge)
		self.edges.sort(key=lambda edge: (
			edge["source"], edge["target"], edge["kind"], json_key(edge)))
		self.node_search_text = {ident: json_key(node).casefold() for ident, node in self.nodes.items()}
		self.edge_search_text = {json_key(edge): json_key(edge).casefold() for edge in self.edges}
		self.edge_fingerprints = {key: hashlib.sha256(key.encode()).hexdigest()
			for key in self.edge_search_text}
		self.outgoing: dict[str, list[dict]] = defaultdict(list)
		self.incoming: dict[str, list[dict]] = defaultdict(list)
		for edge in self.edges:
			self.outgoing[edge["source"]].append(edge)
			self.incoming[edge["target"]].append(edge)

	def node_activity(self, node: dict) -> str:
		if node.get("runtime_active") is False:
			return "inactive"
		for field in ("owner", "function"):
			dependency = node.get(field)
			if isinstance(dependency, str) and self.nodes.get(dependency, {}).get("runtime_active") is False:
				return "inactive"
		return "active" if node.get("runtime_active") is True else "unclassified"

	def node_operational(self, node: dict) -> bool:
		return self.node_activity(node) != "inactive"

	def edge_activity(self, edge: dict) -> str:
		statuses = {self.node_activity(self.nodes[edge[endpoint]]) for endpoint in ("source", "target")}
		if "inactive" in statuses:
			return "inactive"
		return "active" if statuses == {"active"} else "unclassified"

	def edge_operational(self, edge: dict) -> bool:
		return self.edge_activity(edge) != "inactive"

	def evidence_fingerprint(self, edge: dict) -> str:
		return self.edge_fingerprints[json_key(edge)]


def edge_fingerprint(edge: dict) -> str:
	return hashlib.sha256(json_key(edge).encode()).hexdigest()


def graph_fingerprint(path: Path) -> str | None:
	try:
		return hashlib.sha256(path.read_bytes()).hexdigest()
	except OSError:
		return None


def companion_path(graph: Path, explicit: Path | None, filename: str, auto: bool) -> Path | None:
	if explicit is not None:
		if not explicit.is_file():
			raise QueryFailure("missing-companion", f"companion file does not exist: {explicit}")
		return explicit
	candidate = graph.parent / filename
	return candidate if auto and candidate.is_file() else None


def load_companions(args: argparse.Namespace) -> tuple[dict[str, dict | None], dict[str, str]]:
	auto = not args.no_auto_companions
	paths = {
		"coverage": companion_path(args.graph, args.coverage, "coverage.json", auto),
		"completeness": companion_path(args.graph, args.completeness, "completeness.json", auto),
		"query_packs": companion_path(args.graph, args.query_packs, "query-packs.json", auto),
		"component_graph": companion_path(args.graph, args.component_graph, "component-graph.json", auto),
	}
	companions = {name: load_json(path, dict, name.replace("_", "-")) if path else None
		for name, path in paths.items()}
	fingerprints = {name: hashlib.sha256(path.read_bytes()).hexdigest()
		for name, path in paths.items() if path is not None}
	return companions, fingerprints


def warning(code: str, severity: str, message: str, **details: object) -> dict:
	result = {"code": code, "severity": severity, "message": message}
	if details:
		result["details"] = details
	return result


def companion_warnings(companions: dict[str, dict | None], index: GraphIndex | None = None) -> list[dict]:
	result = []
	coverage = companions.get("coverage")
	if coverage is None:
		result.append(warning("coverage-unavailable", "info", "coverage.json was not loaded"))
	else:
		unresolved = coverage.get("unresolved_targets")
		if isinstance(unresolved, int) and unresolved:
			result.append(warning("unresolved-targets", "warning",
				f"coverage reports {unresolved} unresolved runtime targets", count=unresolved))
		inventory = coverage.get("inventory_only_targets")
		if isinstance(inventory, int) and inventory:
			result.append(warning("inventory-only-targets", "info",
				f"coverage reports {inventory} inventory-only targets", count=inventory))
		failed = coverage.get("mir_packages_failed")
		if isinstance(failed, int) and failed:
			result.append(warning("mir-packages-failed", "warning",
				f"coverage reports {failed} failed MIR packages", count=failed))
		if index is not None:
			for field, actual in (("nodes", len(index.nodes)), ("edges", len(index.edges))):
				reported = coverage.get(field)
				if isinstance(reported, int) and reported != actual:
					result.append(warning("companion-count-mismatch", "warning",
						f"coverage {field} count does not match the loaded graph",
						companion="coverage", field=field, reported=reported, actual=actual))
	completeness = companions.get("completeness")
	if completeness is None:
		result.append(warning("completeness-unavailable", "info", "completeness.json was not loaded"))
	else:
		missing = completeness.get("source_components_without_entrypoints")
		if isinstance(missing, list) and missing:
			result.append(warning("components-without-entrypoints", "info",
				f"completeness reports {len(missing)} source components without entrypoints", count=len(missing)))
		historical = completeness.get("historical_properties")
		if isinstance(historical, dict):
			failed_properties = sorted(str(name) for name, present in historical.items() if present is False)
			if failed_properties:
				result.append(warning("historical-properties-missing", "warning",
					f"{len(failed_properties)} historical graph properties are missing",
					properties=failed_properties[:20]))
		path_search = completeness.get("path_search")
		if isinstance(path_search, dict):
			limited = path_search.get("limit_truncated_starts", [])
			depth = path_search.get("depth_truncated_starts", [])
			if isinstance(limited, list) and isinstance(depth, list) and (limited or depth):
				result.append(warning("stored-path-search-truncated", "info",
					"stored path search reports truncated starts",
					limit_truncated=len(limited), depth_truncated=len(depth)))
	query_packs = companions.get("query_packs")
	if query_packs is not None and query_packs.get("schema_version") != 1:
		result.append(warning("query-packs-schema-mismatch", "warning",
			"query-packs.json is missing or does not use schema_version 1",
			expected=1, actual=query_packs.get("schema_version")))
	return sorted(result, key=lambda item: (item["severity"], item["code"], item["message"]))


def activity_warnings(index: GraphIndex) -> list[dict]:
	unclassified_nodes = sum(index.node_activity(node) == "unclassified" for node in index.nodes.values())
	unclassified_edges = sum(index.edge_activity(edge) == "unclassified" for edge in index.edges)
	if not unclassified_nodes and not unclassified_edges:
		return []
	return [warning("runtime-activity-unclassified", "info",
		"missing runtime_active metadata is retained as unclassified and is not treated as explicit active evidence",
		nodes=unclassified_nodes, edges=unclassified_edges,
		operational_policy="exclude only explicit inactive nodes and their dependants")]


def compact_value(value: object, depth: int = 0, max_depth: int = 6,
	max_items: int = 20, max_string: int = 256) -> object:
	if depth >= max_depth and isinstance(value, (dict, list)):
		return {"_truncated": "depth"}
	if isinstance(value, dict):
		items = sorted(value.items(), key=lambda item: str(item[0]))
		result = {str(key): compact_value(item, depth + 1, max_depth, max_items, max_string)
			for key, item in items[:max_items]}
		if len(items) > max_items:
			result["_truncated_fields"] = len(items) - max_items
		return result
	if isinstance(value, list):
		result = [compact_value(item, depth + 1, max_depth, max_items, max_string)
			for item in value[:max_items]]
		if len(value) > max_items:
			result.append({"_truncated_items": len(value) - max_items})
		return result
	if isinstance(value, str) and len(value) > max_string:
		return f"{value[:max_string]}...[{len(value) - max_string} chars omitted]"
	return value


def minimal_node(node: dict) -> dict:
	return {key: node[key] for key in ("id", "kind", "name", "owner", "file", "runtime_active")
		if key in node}


def minimal_edge(edge: dict, evidence_sha256: str | None = None) -> dict:
	return {**{key: edge[key] for key in ("source", "target", "kind", "method", "file", "line")
		if key in edge}, "_evidence_sha256": evidence_sha256 or edge.get("_evidence_sha256")
		or edge_fingerprint(edge)}


def minimal_record(record: dict) -> dict:
	record_type = record.get("record_type")
	if record_type == "node" and isinstance(record.get("node"), dict):
		return {"record_type": "node", "node": minimal_node(record["node"]),
			"runtime_activity": record.get("runtime_activity"), "_record_truncated": True}
	if record_type == "edge" and isinstance(record.get("edge"), dict):
		return {"record_type": "edge",
			"edge": minimal_edge(record["edge"], record.get("evidence_sha256")),
			"runtime_activity": record.get("runtime_activity"),
			"evidence_sha256": record.get("evidence_sha256"), "_record_truncated": True}
	if record_type == "neighbor":
		return {"record_type": "neighbor", "depth": record.get("depth"),
			"from": record.get("from"), "to": record.get("to"),
			"relation": record.get("relation"),
			"node": minimal_node(record.get("node", {})),
			"edge": minimal_edge(record.get("edge", {}), record.get("evidence_sha256")),
			"node_runtime_activity": record.get("node_runtime_activity"),
			"edge_runtime_activity": record.get("edge_runtime_activity"),
			"evidence_sha256": record.get("evidence_sha256"),
			"_record_truncated": True}
	if record_type == "path":
		return {"record_type": "path", "length": record.get("length"), "nodes": record.get("nodes", []),
			"steps": [{"from": step.get("from"), "to": step.get("to"),
				"edge_runtime_activity": step.get("edge_runtime_activity"),
				"edge_kind": step.get("edge_kind"), "edge_kinds": step.get("edge_kinds", []),
				"traversal": step.get("traversal"), "traversals": step.get("traversals", []),
				"evidence_count": step.get("evidence_count"),
				"evidence_sha256": step.get("evidence_sha256"),
				"evidence_sha256s": step.get("evidence_sha256s", []),
				"edge": minimal_edge(step.get("edge", {}), step.get("evidence_sha256"))}
				for step in record.get("steps", [])],
			"_record_truncated": True}
	result = {"_record_truncated": True}
	for key, value in sorted(record.items()):
		if key in {"record_type", "section", "key", "count", "type"} or isinstance(value, (bool, int, float)):
			result[key] = value
	return result


def encode_envelope(envelope: dict) -> tuple[str, int]:
	estimate = 0
	for _ in range(6):
		envelope["budget"]["estimated_tokens"] = estimate
		text = json.dumps(envelope, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
		updated = math.ceil((len(text) + 1) / 4)
		if updated == estimate:
			return text, estimate
		estimate = updated
	envelope["budget"]["estimated_tokens"] = estimate
	text = json.dumps(envelope, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
	return text, math.ceil((len(text) + 1) / 4)


def render_envelope(operation: str, parameters: dict, records: list[dict], matched: int,
	warnings: list[dict], reasons: list[str], max_records: int, max_tokens: int,
	result_metadata: dict | None = None, graph_sha256: str | None = None,
	companion_sha256: dict[str, str] | None = None, total_is_exact: bool = True) -> str:
	reason_set = set(reasons)
	if len(records) > max_records:
		reason_set.add("record-limit")
	candidate_count = min(len(records), max_records)
	raw_query = {"operation": operation, "parameters": parameters}
	query = compact_value(raw_query, max_items=30, max_string=128)
	severity_order = {"error": 0, "warning": 1, "info": 2}
	warnings = sorted(warnings, key=lambda item: (
		severity_order.get(str(item.get("severity")), 3), str(item.get("code")), str(item.get("message"))))
	warnings_selected = [compact_value(item, max_items=10, max_string=160) for item in warnings]
	raw_metadata = result_metadata or {}
	metadata = compact_value(raw_metadata, max_items=20, max_string=128)
	base_field_limited = query != raw_query or metadata != raw_metadata \
		or any(compacted != original for compacted, original in zip(warnings_selected, warnings))
	max_characters = max_tokens * 4 - 1
	companion_sha256 = dict(sorted((companion_sha256 or {}).items()))
	compacted_records: list[dict] = []
	record_field_limited: list[bool] = []

	def prepare(count: int) -> None:
		while len(compacted_records) < count:
			raw = records[len(compacted_records)]
			compacted = compact_value(raw)
			compacted_records.append(compacted)
			record_field_limited.append(compacted != raw)

	def build(selected: list[dict], token_limited: bool,
		forced_field_limit: bool = False) -> dict:
		selected_reasons = set(reason_set)
		if token_limited:
			selected_reasons.add("token-limit")
		if base_field_limited or forced_field_limit \
			or any(record_field_limited[:len(selected)]):
			selected_reasons.add("field-limit")
		return {
			"schema_version": SCHEMA_VERSION,
			"tool": TOOL,
			"graph": {"fingerprint": {"algorithm": "sha256", "value": graph_sha256},
				"companions": companion_sha256},
			"query": query,
			"result": {"matched": matched, "returned": len(selected), "records": selected,
				"total_is_exact": total_is_exact,
				"omitted": max(0, matched - len(selected)) if total_is_exact else None,
				"metadata": metadata},
			"truncated": bool(selected_reasons),
			"truncation_reasons": sorted(selected_reasons),
			"warnings": warnings_selected,
			"budget": {"max_records": max_records, "max_tokens": max_tokens,
				"estimation": "ceil(serialized ASCII characters / 4)", "estimated_tokens": 0},
		}

	def probe(count: int, token_limited: bool, replacement: dict | None = None) -> tuple[bool, str]:
		prepare(count)
		selected = compacted_records[:count]
		forced_field_limit = False
		if replacement is not None and selected:
			selected[-1] = replacement
			forced_field_limit = True
		envelope = build(selected, token_limited, forced_field_limit)
		text, estimated = encode_envelope(envelope)
		return len(text) <= max_characters and estimated <= max_tokens, text

	base_fits, empty_without_token_limit = probe(0, False)
	token_base_fits, empty_with_token_limit = probe(0, True)
	if not token_base_fits:
		while len(warnings_selected) > 1 and not token_base_fits:
			warnings_selected.pop()
			base_field_limited = True
			token_base_fits, empty_with_token_limit = probe(0, True)
		if not token_base_fits:
			query = compact_value(query, max_items=10, max_string=48)
			metadata = compact_value(metadata, max_items=8, max_string=48)
			base_field_limited = True
			token_base_fits, empty_with_token_limit = probe(0, True)
		if not token_base_fits and warnings_selected \
			and warnings_selected[0].get("severity") != "error":
			warnings_selected.clear()
			base_field_limited = True
			token_base_fits, empty_with_token_limit = probe(0, True)
		if not token_base_fits and warnings_selected:
			warnings_selected[:] = [compact_value(warnings_selected[0], max_items=4, max_string=24)]
			base_field_limited = True
			token_base_fits, empty_with_token_limit = probe(0, True)
		if not token_base_fits:
			metadata = {key: value for key, value in metadata.items() if key == "error"}
			base_field_limited = True
			token_base_fits, empty_with_token_limit = probe(0, True)
		if not token_base_fits:
			minimal_parameters = {key: value for key, value in query.get("parameters", {}).items()
				if key == "batch_index"}
			query = {"operation": query.get("operation", operation), "parameters": minimal_parameters}
			base_field_limited = True
			token_base_fits, empty_with_token_limit = probe(0, True)
		if not token_base_fits:
			raise QueryFailure("token-budget-too-small", "token budget cannot fit the response envelope")

	if candidate_count == 0:
		return empty_without_token_limit if base_fits else empty_with_token_limit

	low = 0
	high = 1
	best_text = empty_with_token_limit
	failure = candidate_count
	while low < candidate_count:
		count = min(high, candidate_count)
		fits, text = probe(count, count < candidate_count)
		if fits:
			low = count
			best_text = text
			if count == candidate_count:
				return text
			high = min(candidate_count, count * 2)
			continue
		failure = count
		break

	left, right = low + 1, failure - 1
	while left <= right:
		middle = (left + right) // 2
		fits, text = probe(middle, True)
		if fits:
			low = middle
			best_text = text
			left = middle + 1
		else:
			right = middle - 1

	if low < candidate_count:
		prepare(low + 1)
		minimal = minimal_record(compacted_records[low])
		if minimal != compacted_records[low]:
			fits, text = probe(low + 1, True, minimal)
			if fits:
				return text
	if low:
		fits, text = probe(low, True)
		if fits:
			return text
	return best_text


def require_text(request: dict, field: str) -> str:
	value = request.get(field)
	if not isinstance(value, str) or not value.strip():
		raise QueryFailure("invalid-query", f"{field} must be a non-empty string")
	if len(value) > MAX_QUERY_LENGTH:
		raise QueryFailure("invalid-query", f"{field} exceeds {MAX_QUERY_LENGTH} characters")
	return value


def choice(request: dict, field: str, default: str, allowed: set[str]) -> str:
	value = request.get(field, default)
	if not isinstance(value, str) or value not in allowed:
		raise QueryFailure("invalid-query", f"{field} must be one of: {', '.join(sorted(allowed))}")
	return value


def bounded_int(request: dict, field: str, default: int, minimum: int, maximum: int) -> int:
	value = request.get(field, default)
	if not isinstance(value, int) or isinstance(value, bool) or not minimum <= value <= maximum:
		raise QueryFailure("invalid-query", f"{field} must be an integer from {minimum} to {maximum}")
	return value


def string_list(request: dict, field: str) -> list[str]:
	value = request.get(field, [])
	if value is None:
		return []
	if not isinstance(value, list) or any(not isinstance(item, str) or not item for item in value):
		raise QueryFailure("invalid-query", f"{field} must be an array of non-empty strings")
	return sorted(set(value))


def boolean(request: dict, field: str, default: bool = False) -> bool:
	value = request.get(field, default)
	if not isinstance(value, bool):
		raise QueryFailure("invalid-query", f"{field} must be a boolean")
	return value


def summary(index: GraphIndex, companions: dict[str, dict | None]) -> tuple[list[dict], int, list[str], dict]:
	activity_nodes = Counter(index.node_activity(node) for node in index.nodes.values())
	activity_edges = Counter(index.edge_activity(edge) for edge in index.edges)
	operational_nodes = [node for node in index.nodes.values() if index.node_operational(node)]
	operational_edges = [edge for edge in index.edges if index.edge_operational(edge)]
	records = [
		{"record_type": "summary", "section": "graph",
			"nodes": len(index.nodes), "operational_nodes": len(operational_nodes),
			"edges": len(index.edges), "operational_edges": len(operational_edges),
			"node_runtime_activity": dict(sorted(activity_nodes.items())),
			"edge_runtime_activity": dict(sorted(activity_edges.items()))},
		{"record_type": "summary", "section": "node_kinds",
			"raw": dict(sorted(Counter(node.get("kind", "unknown") for node in index.nodes.values()).items())),
			"operational": dict(sorted(Counter(node.get("kind", "unknown") for node in operational_nodes).items()))},
		{"record_type": "summary", "section": "edge_kinds",
			"raw": dict(sorted(Counter(edge["kind"] for edge in index.edges).items())),
			"operational": dict(sorted(Counter(edge["kind"] for edge in operational_edges).items()))},
		{"record_type": "summary", "section": "domains",
			"raw": dict(sorted(Counter(str(node.get("domain", "unspecified"))
				for node in index.nodes.values()).items())),
			"operational": dict(sorted(Counter(str(node.get("domain", "unspecified"))
				for node in operational_nodes).items()))},
	]
	coverage = companions.get("coverage")
	if coverage is not None:
		records.append({"record_type": "summary", "section": "coverage", "value": coverage})
	completeness = companions.get("completeness")
	if completeness is not None:
		records.append({"record_type": "summary", "section": "completeness",
			"value": {key: value for key, value in completeness.items()
				if key != "source_components_without_entrypoints"},
			"source_components_without_entrypoints": len(
				completeness.get("source_components_without_entrypoints", []))})
	packs = companions.get("query_packs")
	if packs is not None:
		records.append({"record_type": "summary", "section": "query_packs",
			"sections": {key: len(value) if isinstance(value, (list, dict)) else 1
				for key, value in sorted(packs.items()) if key != "schema_version"}})
	return records, len(records), [], {}


def node_details(index: GraphIndex, request: dict) -> tuple[list[dict], int, list[str], dict]:
	ident = require_text(request, "id")
	node = index.nodes.get(ident)
	if node is None:
		raise QueryFailure("node-not-found", f"graph node does not exist: {ident}")
	incoming = index.incoming.get(ident, [])
	outgoing = index.outgoing.get(ident, [])
	record = {"record_type": "node", "node": node,
		"runtime_activity": index.node_activity(node),
		"edge_counts": {
			"incoming": len(incoming), "outgoing": len(outgoing),
			"operational_incoming": sum(index.edge_operational(edge) for edge in incoming),
			"operational_outgoing": sum(index.edge_operational(edge) for edge in outgoing),
			"incoming_by_kind": dict(sorted(Counter(edge["kind"] for edge in incoming).items())),
			"outgoing_by_kind": dict(sorted(Counter(edge["kind"] for edge in outgoing).items())),
		}}
	return [record], 1, [], {}


def search(index: GraphIndex, request: dict) -> tuple[list[dict], int, list[str], dict]:
	term = require_text(request, "text").casefold()
	scope = choice(request, "scope", "nodes", {"nodes", "edges", "all"})
	kinds = set(string_list(request, "kinds"))
	include_inactive = boolean(request, "include_inactive")
	records = []
	if scope in {"nodes", "all"}:
		for ident, node in sorted(index.nodes.items()):
			if not include_inactive and not index.node_operational(node):
				continue
			if kinds and node.get("kind") not in kinds:
				continue
			if term not in index.node_search_text[ident]:
				continue
			folded_id = ident.casefold()
			metadata_fields = ("kind", "name", "owner", "file", "domain", "associated_type",
				"runtime_alias", "config_trait", "entrypoint_kind")
			if folded_id == term:
				rank, reason = 0, "exact-node-id"
			elif folded_id.startswith(term):
				rank, reason = 1, "node-id-prefix"
			elif term in folded_id:
				rank, reason = 2, "node-id-substring"
			elif any(isinstance(node.get(field), str) and node[field].casefold() == term
				for field in metadata_fields):
				rank, reason = 3, "exact-node-metadata"
			else:
				rank, reason = 6, "node-json-substring"
			records.append({"record_type": "node", "runtime_activity": index.node_activity(node),
				"match": {"rank": rank, "reason": reason}, "node": node})
	if scope in {"edges", "all"}:
		for edge in index.edges:
			if not include_inactive and not index.edge_operational(edge):
				continue
			if kinds and edge["kind"] not in kinds:
				continue
			if term not in index.edge_search_text[json_key(edge)]:
				continue
			endpoints = (edge["source"].casefold(), edge["target"].casefold())
			if term in endpoints:
				rank, reason = 4, "exact-edge-endpoint"
			elif any(value.startswith(term) or term in value for value in endpoints):
				rank, reason = 5, "edge-endpoint-substring"
			else:
				rank, reason = 7, "edge-json-substring"
			records.append({"record_type": "edge", "runtime_activity": index.edge_activity(edge),
				"match": {"rank": rank, "reason": reason},
				"evidence_sha256": index.evidence_fingerprint(edge), "edge": edge})
	records.sort(key=lambda record: (record["match"]["rank"],
		record.get("node", {}).get("id", ""), record.get("edge", {}).get("source", ""),
		record.get("edge", {}).get("target", ""), json_key(record)))
	return records, len(records), [], {"include_inactive": include_inactive,
		"activity_policy": ACTIVITY_POLICY}


def neighbors(index: GraphIndex, request: dict) -> tuple[list[dict], int, list[str], dict]:
	ident = require_text(request, "id")
	if ident not in index.nodes:
		raise QueryFailure("node-not-found", f"graph node does not exist: {ident}")
	direction = choice(request, "direction", "both", {"incoming", "outgoing", "both"})
	depth = bounded_int(request, "depth", 1, 1, 6)
	max_expansions = bounded_int(request, "max_expansions", 10_000, 1, 100_000)
	kinds = set(string_list(request, "edge_kinds"))
	include_inactive = boolean(request, "include_inactive")
	records = []
	seen_evidence = set()
	discovered = {ident: 0}
	queue = deque([ident])
	expansions = 0
	reasons = []
	expansion_limited = False
	while queue and not expansion_limited:
		current = queue.popleft()
		current_depth = discovered[current]
		if current_depth >= depth:
			continue
		for neighbor, edge, traversal in path_transitions(index, current, direction, kinds, include_inactive):
			fingerprint = index.evidence_fingerprint(edge)
			if fingerprint in seen_evidence:
				continue
			if expansions >= max_expansions:
				expansion_limited = True
				break
			seen_evidence.add(fingerprint)
			expansions += 1
			relation = "self" if neighbor == current else "outgoing" if traversal == "forward" else "incoming"
			records.append({"record_type": "neighbor", "depth": current_depth + 1,
				"from": current, "to": neighbor, "relation": relation,
				"node": index.nodes[neighbor],
				"node_runtime_activity": index.node_activity(index.nodes[neighbor]),
				"edge_runtime_activity": index.edge_activity(edge),
				"evidence_sha256": fingerprint, "edge": edge})
			if neighbor not in discovered:
				discovered[neighbor] = current_depth + 1
				queue.append(neighbor)
	if expansion_limited:
		reasons.append("expansion-limit")
	if not expansion_limited and any(
		index.evidence_fingerprint(edge) not in seen_evidence
		for frontier, frontier_depth in discovered.items() if frontier_depth == depth
		for _, edge, _ in path_transitions(index, frontier, direction, kinds, include_inactive)
	):
		reasons.append("depth-limit")
	records.sort(key=lambda record: (record["depth"], record["from"], record["to"],
		record["relation"], json_key(record["edge"])))
	return records, len(records), reasons, {"direction": direction, "depth": depth,
		"include_inactive": include_inactive, "max_expansions": max_expansions,
		"expansions": expansions, "nodes_reached": len(discovered), "search_complete": not reasons,
		"activity_policy": ACTIVITY_POLICY}


def path_transitions(index: GraphIndex, node: str, direction: str, kinds: set[str],
	include_inactive: bool) -> list[tuple[str, dict, str]]:
	result = []
	if direction in {"outgoing", "both"}:
		result.extend((edge["target"], edge, "forward") for edge in index.outgoing.get(node, []))
	if direction in {"incoming", "both"}:
		result.extend((edge["source"], edge, "reverse") for edge in index.incoming.get(node, []))
	filtered = []
	seen = set()
	for neighbor, edge, traversal in result:
		if kinds and edge["kind"] not in kinds:
			continue
		if not include_inactive and not index.edge_operational(edge):
			continue
		key = (neighbor, traversal, json_key(edge))
		if key not in seen:
			seen.add(key)
			filtered.append((neighbor, edge, traversal))
	return sorted(filtered, key=lambda item: (item[0], item[2], json_key(item[1])))


def aggregated_path_transitions(index: GraphIndex, node: str, direction: str, kinds: set[str],
	include_inactive: bool) -> list[tuple[str, list[tuple[dict, tuple[str, ...]]]]]:
	groups: dict[str, dict[str, dict]] = defaultdict(dict)
	traversals: dict[tuple[str, str], set[str]] = defaultdict(set)
	for neighbor, edge, traversal in path_transitions(index, node, direction, kinds, include_inactive):
		fingerprint = index.evidence_fingerprint(edge)
		groups[neighbor][fingerprint] = edge
		traversals[(neighbor, fingerprint)].add(traversal)
	return [(neighbor, [(edges[fingerprint], tuple(sorted(traversals[(neighbor, fingerprint)])))
		for fingerprint in sorted(edges)]) for neighbor, edges in sorted(groups.items())]


def paths(index: GraphIndex, request: dict) -> tuple[list[dict], int, list[str], dict]:
	source = require_text(request, "source")
	target = require_text(request, "target")
	if source not in index.nodes:
		raise QueryFailure("node-not-found", f"graph node does not exist: {source}")
	if target not in index.nodes:
		raise QueryFailure("node-not-found", f"graph node does not exist: {target}")
	direction = choice(request, "direction", "outgoing", {"incoming", "outgoing", "both"})
	max_depth = bounded_int(request, "max_depth", 6, 0, 12)
	max_paths = bounded_int(request, "max_paths", 10, 1, 1_000)
	max_expansions = bounded_int(request, "max_expansions", 10_000, 1, 100_000)
	kinds = set(string_list(request, "edge_kinds"))
	include_inactive = boolean(request, "include_inactive")
	distances = {source: 0}
	predecessors: dict[str, list[tuple[str, list[tuple[dict, tuple[str, ...]]]]]] = defaultdict(list)
	queue = deque([source])
	expansions = 0
	depth_truncated = False
	expansion_limited = False
	reasons = []
	target_depth = 0 if source == target else None
	while queue and not expansion_limited:
		current = queue.popleft()
		current_depth = distances[current]
		if target_depth is not None and current_depth >= target_depth:
			continue
		transitions = aggregated_path_transitions(index, current, direction, kinds, include_inactive)
		if current_depth >= max_depth:
			depth_truncated = depth_truncated or any(neighbor not in distances
				for neighbor, _ in transitions)
			continue
		for neighbor, evidence in transitions:
			if expansions >= max_expansions:
				expansion_limited = True
				break
			expansions += 1
			if neighbor not in distances:
				distances[neighbor] = current_depth + 1
				queue.append(neighbor)
				if neighbor == target:
					target_depth = current_depth + 1
			if distances.get(neighbor) == current_depth + 1:
				predecessors[neighbor].append((current, evidence))
	if expansion_limited:
		reasons.append("expansion-limit")
	if target_depth is None and depth_truncated:
		reasons.append("depth-limit")
	found = []
	path_queue = deque([(target, [target], [])]) if target_depth is not None else deque()
	reconstruction_limited = False
	while path_queue and len(found) < max_paths and not reconstruction_limited:
		current, reversed_nodes, reversed_steps = path_queue.popleft()
		if current == source:
			found.append({"record_type": "path", "length": len(reversed_steps),
				"nodes": list(reversed(reversed_nodes)), "steps": list(reversed(reversed_steps))})
			continue
		for previous, evidence in predecessors.get(current, []):
			if expansions >= max_expansions:
				reconstruction_limited = True
				break
			expansions += 1
			edges = [edge for edge, _ in evidence]
			fingerprints = [index.evidence_fingerprint(edge) for edge in edges]
			traversal_values = sorted({value for _, values in evidence for value in values})
			edge_kinds = sorted({edge["kind"] for edge in edges})
			activities = sorted({index.edge_activity(edge) for edge in edges})
			step = {"from": previous, "to": current,
				"traversal": traversal_values[0] if len(traversal_values) == 1 else "mixed",
				"traversals": traversal_values,
				"edge_kind": edge_kinds[0] if len(edge_kinds) == 1 else "mixed",
				"edge_kinds": edge_kinds,
				"edge_runtime_activity": activities[0] if len(activities) == 1 else "mixed",
				"evidence_count": len(edges), "evidence_sha256": fingerprints[0],
				"evidence_sha256s": fingerprints, "edge": edges[0],
				"evidence_variants": [{"traversals": list(values),
					"evidence_sha256": fingerprint, "edge": edge}
					for (edge, values), fingerprint in zip(evidence, fingerprints)]}
			path_queue.append((previous, [*reversed_nodes, previous], [*reversed_steps, step]))
	if len(found) >= max_paths and path_queue:
		reasons.append("path-limit")
	if reconstruction_limited and "expansion-limit" not in reasons:
		reasons.append("expansion-limit")
	return found, len(found), reasons, {"direction": direction, "include_inactive": include_inactive,
		"max_depth": max_depth, "max_paths": max_paths, "max_expansions": max_expansions,
		"expansions": expansions, "nodes_reached": len(distances), "shortest_depth": target_depth,
		"path_strategy": "all-shortest unique node paths; all parallel evidence grouped per topology hop",
		"search_complete": not reasons,
		"activity_policy": ACTIVITY_POLICY}


def component_index(index: GraphIndex, payload: dict | None) -> GraphIndex | None:
	if payload is None:
		return None
	if payload.get("schema_version") != 2:
		raise QueryFailure("invalid-component-graph", "component-graph.json must use schema_version 2")
	if payload.get("projection") != "execution":
		raise QueryFailure("invalid-component-graph",
			"component-graph.json must use the execution projection")
	edges = payload.get("edges")
	if not isinstance(edges, list):
		raise QueryFailure("invalid-component-graph", "component-graph.json must contain an edge array")
	if any(not isinstance(edge, dict) or any(
		not isinstance(edge.get(field), str) or not edge[field]
		for field in ("source", "target", "kind")) for edge in edges):
		raise QueryFailure("invalid-component-graph",
			"every component edge must have source, target, and kind strings")
	node_ids = {edge.get(endpoint) for edge in edges if isinstance(edge, dict)
		for endpoint in ("source", "target")}
	missing = sorted((ident for ident in node_ids
		if not isinstance(ident, str) or ident not in index.nodes), key=lambda value: str(value))
	if missing:
		raise QueryFailure("invalid-component-graph",
			f"component graph contains nodes absent from the main graph: {missing[0]}")
	return GraphIndex({"schema_version": 2,
		"nodes": [index.nodes[ident] for ident in sorted(node_ids)], "edges": edges})


def packs(companions: dict[str, dict | None], request: dict) -> tuple[list[dict], int, list[str], dict]:
	payload = companions.get("query_packs")
	if payload is None:
		raise QueryFailure("query-packs-unavailable", "query-packs.json was not loaded")
	section = request.get("section")
	contains = request.get("contains")
	if section is not None and (not isinstance(section, str) or not section):
		raise QueryFailure("invalid-query", "section must be a non-empty string")
	if contains is not None and (not isinstance(contains, str) or len(contains) > MAX_QUERY_LENGTH):
		raise QueryFailure("invalid-query", f"contains must be a string up to {MAX_QUERY_LENGTH} characters")
	if section is None:
		records = [{"record_type": "query-pack", "section": key,
			"type": "list" if isinstance(value, list) else "object" if isinstance(value, dict) else "scalar",
			"count": len(value) if isinstance(value, (list, dict)) else 1}
			for key, value in sorted(payload.items()) if key != "schema_version"]
	else:
		if section == "schema_version" or section not in payload:
			raise QueryFailure("section-not-found", f"query-pack section does not exist: {section}")
		value = payload[section]
		if isinstance(value, list):
			records = [{"record_type": "query-pack", "section": section, "index": index, "value": item}
				for index, item in enumerate(value)]
		elif isinstance(value, dict):
			records = [{"record_type": "query-pack", "section": section, "key": key, "value": item}
				for key, item in sorted(value.items())]
		else:
			records = [{"record_type": "query-pack", "section": section, "value": value}]
	if contains:
		needle = contains.casefold()
		records = [record for record in records if needle in json_key(record).casefold()]
	return records, len(records), [], {"section": section, "contains": contains}


def execute(index: GraphIndex, companions: dict[str, dict | None], request: dict,
	max_records: int, max_tokens: int, graph_sha256: str | None,
	component_graph_index: GraphIndex | None = None, batch_index: int | None = None,
	companion_sha256: dict[str, str] | None = None) -> str:
	operation = request.get("operation")
	if not isinstance(operation, str) or operation not in {
		"summary", "node", "search", "neighbors", "paths", "packs"}:
		raise QueryFailure("invalid-operation",
			"operation must be one of: neighbors, node, packs, paths, search, summary")
	def path_query() -> tuple[list[dict], int, list[str], dict]:
		view = choice(request, "view", "raw", {"raw", "components"})
		if view == "components" and component_graph_index is None:
			raise QueryFailure("component-graph-unavailable",
				"component path queries require component-graph.json")
		result = paths(component_graph_index if view == "components" else index, request)
		result[3]["view"] = view
		return result

	dispatch = {
		"summary": lambda: summary(index, companions),
		"node": lambda: node_details(index, request),
		"search": lambda: search(index, request),
		"neighbors": lambda: neighbors(index, request),
		"paths": path_query,
		"packs": lambda: packs(companions, request),
	}
	records, matched, reasons, metadata = dispatch[operation]()
	parameters = {key: value for key, value in sorted(request.items()) if key != "operation"}
	if batch_index is not None:
		parameters["batch_index"] = batch_index
	warnings = [*companion_warnings(companions, index), *activity_warnings(index)]
	total_is_exact = not bool(set(reasons) & {"depth-limit", "expansion-limit", "path-limit"})
	return render_envelope(operation, parameters, records, matched, warnings,
		reasons, max_records, max_tokens, metadata, graph_sha256,
		companion_sha256, total_is_exact)


def error_response(operation: str, parameters: dict, error: QueryFailure,
	companions: dict[str, dict | None], max_records: int, max_tokens: int,
	graph_sha256: str | None = None, companion_sha256: dict[str, str] | None = None,
	index: GraphIndex | None = None) -> str:
	warnings = [*companion_warnings(companions, index), warning(error.code, "error", str(error))]
	return render_envelope(operation, parameters, [], 0, warnings, [], max_records, max_tokens,
		{"error": error.code}, graph_sha256, companion_sha256)


def request_from_args(args: argparse.Namespace) -> dict:
	request = {"operation": args.operation}
	for field in ("id", "text", "scope", "direction", "view", "source", "target", "depth", "max_depth", "max_paths",
		"max_expansions", "section", "contains"):
		if hasattr(args, field) and getattr(args, field) is not None:
			request[field] = getattr(args, field)
	if hasattr(args, "kind") and args.kind:
		request["kinds"] = args.kind
	if hasattr(args, "edge_kind") and args.edge_kind:
		request["edge_kinds"] = args.edge_kind
	if getattr(args, "include_inactive", False):
		request["include_inactive"] = True
	return request


def batch(index: GraphIndex, companions: dict[str, dict | None], args: argparse.Namespace,
	graph_sha256: str | None, component_graph_index: GraphIndex | None,
	companion_sha256: dict[str, str]) -> int:
	failed = False
	for line_number, raw in enumerate(sys.stdin, 1):
		if not raw.strip():
			continue
		operation = "invalid"
		parameters = {"batch_index": line_number}
		request_records = args.max_records
		request_tokens = args.max_tokens
		try:
			request = json.loads(raw)
			if not isinstance(request, dict):
				raise QueryFailure("invalid-batch-request", "each JSONL request must be an object")
			candidate_records = request.get("max_records", args.max_records)
			if isinstance(candidate_records, int) and not isinstance(candidate_records, bool) \
				and 1 <= candidate_records <= args.max_records:
				request_records = candidate_records
			candidate_tokens = request.get("max_tokens", args.max_tokens)
			if isinstance(candidate_tokens, int) and not isinstance(candidate_tokens, bool) \
				and 256 <= candidate_tokens <= args.max_tokens:
				request_tokens = candidate_tokens
			request_records = bounded_int(request, "max_records", args.max_records, 1, args.max_records)
			request_tokens = bounded_int(request, "max_tokens", args.max_tokens, 256, args.max_tokens)
			operation_value = request.get("operation")
			if not isinstance(operation_value, str):
				raise QueryFailure("invalid-operation",
					"operation must be one of: neighbors, node, packs, paths, search, summary")
			operation = operation_value
			print(execute(index, companions, request, request_records, request_tokens,
				graph_sha256, component_graph_index, line_number, companion_sha256))
		except (json.JSONDecodeError, QueryFailure, TypeError, KeyError, AttributeError, OverflowError) as error:
			failed = True
			if isinstance(error, QueryFailure):
				failure = error
			elif isinstance(error, json.JSONDecodeError):
				failure = QueryFailure("invalid-batch-json",
					f"invalid JSON on batch line {line_number}: {error}")
			else:
				failure = QueryFailure("invalid-batch-request",
					f"invalid request on batch line {line_number}: {error}")
			print(error_response(operation, parameters, failure, companions,
				request_records, request_tokens, graph_sha256, companion_sha256, index))
	return 2 if failed else 0


def parser() -> argparse.ArgumentParser:
	result = argparse.ArgumentParser(description="Deterministically query a Hydration runtime interaction graph")
	result.add_argument("--graph", type=Path, required=True, help="interaction-graph.json")
	result.add_argument("--coverage", type=Path, help="optional coverage.json; sibling is auto-loaded")
	result.add_argument("--completeness", type=Path, help="optional completeness.json; sibling is auto-loaded")
	result.add_argument("--query-packs", type=Path, help="optional query-packs.json; sibling is auto-loaded")
	result.add_argument("--component-graph", type=Path,
		help="optional component-graph.json; sibling is auto-loaded")
	result.add_argument("--no-auto-companions", action="store_true", help="do not auto-load sibling companions")
	result.add_argument("--max-records", type=int, default=DEFAULT_MAX_RECORDS)
	result.add_argument("--max-tokens", type=int, default=DEFAULT_MAX_TOKENS,
		help="approximate output token budget")
	subparsers = result.add_subparsers(dest="operation", required=True)
	subparsers.add_parser("summary", help="graph and companion coverage summary")
	node_parser = subparsers.add_parser("node", help="full metadata and edge counts for one node")
	node_parser.add_argument("id")
	search_parser = subparsers.add_parser("search", help="search canonical node or edge JSON")
	search_parser.add_argument("text")
	search_parser.add_argument("--scope", choices=("nodes", "edges", "all"), default="nodes")
	search_parser.add_argument("--kind", action="append", help="repeatable node or edge kind filter")
	search_parser.add_argument("--include-inactive", action="store_true")
	neighbor_parser = subparsers.add_parser("neighbors", help="bounded incoming and outgoing ego subgraph")
	neighbor_parser.add_argument("id")
	neighbor_parser.add_argument("--direction", choices=("incoming", "outgoing", "both"), default="both")
	neighbor_parser.add_argument("--depth", type=int, default=1)
	neighbor_parser.add_argument("--max-expansions", type=int, default=10_000)
	neighbor_parser.add_argument("--edge-kind", action="append")
	neighbor_parser.add_argument("--include-inactive", action="store_true")
	path_parser = subparsers.add_parser("paths", help="bounded deterministic simple-path search")
	path_parser.add_argument("source")
	path_parser.add_argument("target")
	path_parser.add_argument("--direction", choices=("incoming", "outgoing", "both"), default="outgoing")
	path_parser.add_argument("--view", choices=("raw", "components"), default="raw")
	path_parser.add_argument("--edge-kind", action="append")
	path_parser.add_argument("--max-depth", type=int, default=6)
	path_parser.add_argument("--max-paths", type=int, default=10)
	path_parser.add_argument("--max-expansions", type=int, default=10_000)
	path_parser.add_argument("--include-inactive", action="store_true")
	packs_parser = subparsers.add_parser("packs", help="list or inspect query-pack sections")
	packs_parser.add_argument("--section")
	packs_parser.add_argument("--contains")
	subparsers.add_parser("batch", help="read JSON query objects from stdin and emit JSONL envelopes")
	return result


def main() -> int:
	args = parser().parse_args()
	if not 1 <= args.max_records <= 10_000:
		parser().error("--max-records must be from 1 to 10000")
	if not 256 <= args.max_tokens <= 1_000_000:
		parser().error("--max-tokens must be from 256 to 1000000")
	companions: dict[str, dict | None] = {"coverage": None, "completeness": None,
		"query_packs": None, "component_graph": None}
	companion_sha256: dict[str, str] = {}
	graph_sha256 = graph_fingerprint(args.graph)
	component_graph_index = None
	index = None
	try:
		payload = load_json(args.graph, dict, "graph")
		index = GraphIndex(payload)
		companions, companion_sha256 = load_companions(args)
		component_graph_index = component_index(index, companions.get("component_graph"))
	except QueryFailure as error:
		print(error_response(args.operation, {}, error, companions, args.max_records, args.max_tokens,
			graph_sha256, companion_sha256, index))
		return 2
	if args.operation == "batch":
		return batch(index, companions, args, graph_sha256, component_graph_index, companion_sha256)
	request = request_from_args(args)
	try:
		print(execute(index, companions, request, args.max_records, args.max_tokens,
			graph_sha256, component_graph_index, companion_sha256=companion_sha256))
		return 0
	except QueryFailure as error:
		print(error_response(args.operation, {key: value for key, value in request.items()
			if key != "operation"}, error, companions, args.max_records, args.max_tokens,
			graph_sha256, companion_sha256, index))
		return 2


if __name__ == "__main__":
	raise SystemExit(main())
