#!/usr/bin/env python3

import argparse
import json
from pathlib import Path


VOLATILE_FIELDS = frozenset({
	"artifact",
	"binding_file",
	"configured_by",
	"declaration_file",
	"file",
	"first_external_line",
	"impl_line",
	"line",
	"manifest",
	"mir_source",
	"path",
	"rapx_output",
	"source_file",
})
RISKY_KINDS = frozenset({
	"affects-invariant",
	"asset-operation",
	"authorizes-entry",
	"burns",
	"dispatches-frame",
	"dynamic-call",
	"enforces",
	"enters-evm",
	"external-execution",
	"guards",
	"locks",
	"mints",
	"mir-dispatches-frame",
	"mir-dynamic-call",
	"mir-enters-evm",
	"mir-external-call",
	"must-equal",
	"nested-dispatch",
	"proxy-implementation",
	"runtime-configures-contract",
	"transfers-from",
	"transfers-to",
})
RISKY_ENDPOINT_PREFIXES = ("boundary:", "guard:", "invariant:", "operation:", "origin:", "precompile:")
SEMANTIC_DETAIL_FIELDS = ("operation", "value", "enforcement", "resolution")


def identity(edge: dict) -> tuple:
	return edge["source"], edge["target"], edge["kind"], edge.get("method")


def identity_key(value: tuple) -> tuple:
	return tuple((item is None, "" if item is None else str(item)) for item in value)


def canonical(value):
	if isinstance(value, dict):
		return {key: canonical(item) for key, item in sorted(value.items()) if key not in VOLATILE_FIELDS}
	if isinstance(value, list):
		return [canonical(item) for item in value]
	return value


def fingerprint(value: dict) -> str:
	return json.dumps(canonical(value), sort_keys=True, separators=(",", ":"), default=str)


def display(value: dict) -> dict:
	result = canonical(value)
	location = {key: value[key] for key in sorted(VOLATILE_FIELDS & value.keys()) if value[key] is not None}
	if location:
		result["location"] = location
	return result


def edge_index(edges: list[dict]) -> dict[tuple, dict[str, dict]]:
	result = {}
	for edge in sorted(edges, key=lambda item: (
		identity_key(identity(item)),
		item.get("line") is None,
		item.get("line") or 0,
		item.get("file") or "",
	)):
		result.setdefault(identity(edge), {}).setdefault(fingerprint(edge), edge)
	return result


def review_priority(edge: dict) -> bool:
	return edge["kind"] in RISKY_KINDS or edge["source"].startswith(RISKY_ENDPOINT_PREFIXES) \
		or edge["target"].startswith(RISKY_ENDPOINT_PREFIXES)


def semantic_details(edge: dict) -> str:
	return ", ".join(f"{field}={edge[field]}" for field in SEMANTIC_DETAIL_FIELDS
		if edge.get(field) is not None)


def variant_details(edges: list[dict]) -> str:
	details = [semantic_details(edge) or "semantic variant" for edge in edges]
	return " | ".join(details)


def diff(before: dict, after: dict) -> dict:
	left_nodes = {node["id"]: node for node in before["nodes"]}
	right_nodes = {node["id"]: node for node in after["nodes"]}
	nodes_changed = []
	for ident in sorted(left_nodes.keys() & right_nodes.keys()):
		if fingerprint(left_nodes[ident]) == fingerprint(right_nodes[ident]):
			continue
		before_node = canonical(left_nodes[ident])
		after_node = canonical(right_nodes[ident])
		before_node.pop("id", None)
		after_node.pop("id", None)
		nodes_changed.append({"id": ident, "before": before_node, "after": after_node})

	left_edges = edge_index(before["edges"])
	right_edges = edge_index(after["edges"])
	added = []
	removed = []
	edges_changed = []
	for ident in sorted(left_edges.keys() | right_edges.keys(), key=identity_key):
		left_variants = left_edges.get(ident, {})
		right_variants = right_edges.get(ident, {})
		left_only = sorted(left_variants.keys() - right_variants.keys())
		right_only = sorted(right_variants.keys() - left_variants.keys())
		if left_only and right_only:
			edges_changed.append({
				"source": ident[0],
				"target": ident[1],
				"kind": ident[2],
				"method": ident[3],
				"before": [display(left_variants[key]) for key in left_only],
				"after": [display(right_variants[key]) for key in right_only],
			})
		elif left_only:
			removed.extend(display(left_variants[key]) for key in left_only)
		elif right_only:
			added.extend(display(right_variants[key]) for key in right_only)
	return {
		"nodes_added": sorted(right_nodes.keys() - left_nodes.keys()),
		"nodes_removed": sorted(left_nodes.keys() - right_nodes.keys()),
		"nodes_changed": nodes_changed,
		"edges_added": added,
		"edges_removed": removed,
		"edges_changed": edges_changed,
		"review_edges_added": [edge for edge in added if review_priority(edge)],
		"review_edges_removed": [edge for edge in removed if review_priority(edge)],
		"review_edges_changed": [edge for edge in edges_changed if review_priority(edge)],
	}


def compare_coverage(before: dict, after: dict, thresholds: dict) -> dict:
	changes = {field: after[field] - before[field] for field in sorted(before.keys() & after.keys())
		if isinstance(before[field], (int, float)) and isinstance(after[field], (int, float))}
	regressions = []
	for field, minimum in thresholds.get("minimum", {}).items():
		if after.get(field) is None or after[field] < minimum:
			regressions.append(f"{field}={after.get(field)} is below minimum {minimum}")
	for field, maximum in thresholds.get("maximum", {}).items():
		if after.get(field) is None or after[field] > maximum:
			regressions.append(f"{field}={after.get(field)} is above maximum {maximum}")
	for field, expected in thresholds.get("exact", {}).items():
		if after.get(field) != expected:
			regressions.append(f"{field}={after.get(field)!r} does not equal {expected!r}")
	for field, maximum_drop in thresholds.get("regression", {}).get("maximum_drop", {}).items():
		if before.get(field) is None or after.get(field) is None:
			regressions.append(f"{field} is unavailable for regression comparison")
		elif before[field] - after[field] > maximum_drop:
			regressions.append(f"{field} dropped by {before[field] - after[field]}, maximum is {maximum_drop}")
	for field, maximum_increase in thresholds.get("regression", {}).get("maximum_increase", {}).items():
		if before.get(field) is None or after.get(field) is None:
			regressions.append(f"{field} is unavailable for regression comparison")
		elif after[field] - before[field] > maximum_increase:
			regressions.append(f"{field} increased by {after[field] - before[field]}, maximum is {maximum_increase}")
	return {"before": before, "after": after, "changes": changes, "regressions": regressions}


def markdown(result: dict) -> str:
	lines = ["# Runtime interaction graph diff", "",
		f"- Nodes: +{len(result['nodes_added'])} / -{len(result['nodes_removed'])} / "
		f"~{len(result['nodes_changed'])}",
		f"- Edges: +{len(result['edges_added'])} / -{len(result['edges_removed'])} / "
		f"~{len(result['edges_changed'])}",
		f"- Review-priority edges: +{len(result['review_edges_added'])} / "
		f"-{len(result['review_edges_removed'])} / ~{len(result['review_edges_changed'])}", ""]
	coverage = result.get("coverage")
	if coverage:
		lines.extend(["## Coverage changes", ""])
		lines.extend(f"- `{field}`: {change:+}" for field, change in coverage["changes"].items())
		if coverage["regressions"]:
			lines.extend(["", "## Coverage regressions", ""])
			lines.extend(f"- {failure}" for failure in coverage["regressions"])
		lines.append("")
	if result["nodes_changed"]:
		lines.extend(["## Changed nodes", ""])
		for change in result["nodes_changed"][:200]:
			before_kind = change["before"].get("kind")
			after_kind = change["after"].get("kind")
			transition = f" (`{before_kind}` → `{after_kind}`)" if before_kind != after_kind else ""
			lines.append(f"- `{change['id']}`{transition}")
		lines.append("")
	for key, heading in (
		("review_edges_added", "New review-priority edges"),
		("review_edges_removed", "Removed review-priority edges"),
	):
		if not result[key]:
			continue
		lines.extend([f"## {heading}", ""])
		for edge in result[key][:200]:
			line = edge.get("location", {}).get("line")
			where = f" line {line}" if line else ""
			via = f"::{edge['method']}" if edge.get("method") else ""
			details = semantic_details(edge)
			detail = f", {details}" if details else ""
			lines.append(f"- `{edge['source']}` → `{edge['target']}{via}` (`{edge['kind']}`{where}{detail})")
		lines.append("")
	if result["review_edges_changed"]:
		lines.extend(["## Changed review-priority edges", ""])
		for change in result["review_edges_changed"][:200]:
			via = f"::{change['method']}" if change.get("method") else ""
			lines.append(f"- `{change['source']}` → `{change['target']}{via}` (`{change['kind']}`; "
				f"before: {variant_details(change['before'])}; after: {variant_details(change['after'])})")
		lines.append("")
	return "\n".join(lines) + "\n"


def main() -> None:
	parser = argparse.ArgumentParser()
	parser.add_argument("before", type=Path)
	parser.add_argument("after", type=Path)
	parser.add_argument("--output", type=Path)
	parser.add_argument("--markdown", type=Path)
	parser.add_argument("--before-coverage", type=Path)
	parser.add_argument("--after-coverage", type=Path)
	parser.add_argument("--coverage-thresholds", type=Path)
	args = parser.parse_args()
	difference = diff(json.loads(args.before.read_text()), json.loads(args.after.read_text()))
	coverage_options = (args.before_coverage, args.after_coverage, args.coverage_thresholds)
	if any(coverage_options) and not all(coverage_options):
		parser.error("coverage comparison requires --before-coverage, --after-coverage, and --coverage-thresholds")
	if all(coverage_options):
		difference["coverage"] = compare_coverage(
			json.loads(args.before_coverage.read_text()),
			json.loads(args.after_coverage.read_text()),
			json.loads(args.coverage_thresholds.read_text()),
		)
	result = json.dumps(difference, indent=2) + "\n"
	if args.output:
		args.output.write_text(result)
	else:
		print(result, end="")
	if args.markdown:
		args.markdown.write_text(markdown(difference))
	if difference.get("coverage", {}).get("regressions"):
		failures = "; ".join(difference["coverage"]["regressions"])
		raise SystemExit("coverage regression check failed: " + failures)


if __name__ == "__main__":
	main()
