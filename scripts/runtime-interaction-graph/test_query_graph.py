import hashlib
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


MODULE_PATH = Path(__file__).with_name("query_graph.py")
SPEC = importlib.util.spec_from_file_location("runtime_graph_query", MODULE_PATH)
query_graph = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(query_graph)


def fixture_graph() -> dict:
	return {
		"schema_version": 2,
		"nodes": [
			{"id": "a", "kind": "pallet", "domain": "frame", "runtime_active": True},
			{"id": "b", "kind": "function", "owner": "a", "name": "bridge"},
			{"id": "c", "kind": "pallet", "runtime_active": False},
			{"id": "d", "kind": "function", "owner": "c", "name": "inherited-inactive"},
			{"id": "e", "kind": "execution-boundary", "domain": "evm", "runtime_active": True},
		],
		"edges": [
			{"source": "a", "target": "b", "kind": "call", "file": "one.rs", "line": 1},
			{"source": "a", "target": "b", "kind": "call", "file": "two.rs", "line": 2},
			{"source": "b", "target": "e", "kind": "enters-evm"},
			{"source": "c", "target": "a", "kind": "inactive-call"},
			{"source": "a", "target": "d", "kind": "inactive-call"},
			{"source": "e", "target": "a", "kind": "cycle"},
		],
	}


def fixture_companions() -> dict:
	return {
		"coverage": {"unresolved_targets": 1, "inventory_only_targets": 2,
			"mir_packages_failed": 1},
		"completeness": {"source_components_without_entrypoints": ["pallet:x"],
			"historical_properties": {"expected-path": False},
			"path_search": {"limit_truncated_starts": ["a"], "depth_truncated_starts": []}},
		"query_packs": {"schema_version": 1, "cycles": [["a", "b"]], "origins": {"root": ["a"]}},
	}


class QueryGraphTests(unittest.TestCase):
	def setUp(self):
		self.index = query_graph.GraphIndex(fixture_graph())
		self.companions = fixture_companions()
		self.fingerprint = "a" * 64

	def execute(self, request: dict, max_records: int = 50, max_tokens: int = 4_000) -> dict:
		return json.loads(query_graph.execute(
			self.index, self.companions, request, max_records, max_tokens, self.fingerprint))

	def test_summary_should_use_stable_envelope_and_expose_companion_warnings(self):
		request = {"operation": "summary"}
		first = query_graph.execute(self.index, self.companions, request, 50, 4_000, self.fingerprint)
		second = query_graph.execute(self.index, self.companions, request, 50, 4_000, self.fingerprint)
		self.assertEqual(first, second)
		payload = json.loads(first)
		self.assertEqual(payload["schema_version"], 1)
		self.assertEqual(payload["tool"], query_graph.TOOL)
		self.assertEqual(payload["graph"]["fingerprint"]["value"], self.fingerprint)
		self.assertEqual(payload["graph"]["companions"], {})
		self.assertEqual(payload["result"]["matched"], payload["result"]["returned"])
		self.assertTrue(payload["result"]["total_is_exact"])
		graph = next(record for record in payload["result"]["records"] if record["section"] == "graph")
		self.assertEqual(graph["node_runtime_activity"], {"active": 2, "inactive": 2, "unclassified": 1})
		codes = {item["code"] for item in payload["warnings"]}
		self.assertTrue({"unresolved-targets", "mir-packages-failed", "historical-properties-missing",
			"runtime-activity-unclassified"}.issubset(codes))
		self.assertLessEqual(payload["budget"]["estimated_tokens"], payload["budget"]["max_tokens"])

	def test_search_should_exclude_only_explicit_or_inherited_inactive_nodes(self):
		default = self.execute({"operation": "search", "text": "bridge"})
		self.assertEqual([record["node"]["id"] for record in default["result"]["records"]], ["b"])
		self.assertEqual(default["result"]["records"][0]["runtime_activity"], "unclassified")
		explicit = self.execute({"operation": "search", "text": "c", "include_inactive": True})
		self.assertEqual(explicit["result"]["records"][0]["runtime_activity"], "inactive")
		inherited = self.execute({"operation": "search", "text": "inherited-inactive",
			"include_inactive": True})
		self.assertEqual(inherited["result"]["records"][0]["runtime_activity"], "inactive")
		with self.assertRaisesRegex(query_graph.QueryFailure, "must be a boolean"):
			query_graph.search(self.index, {"text": "bridge", "include_inactive": "false"})
		ranked = self.execute({"operation": "search", "text": "a", "scope": "all"})
		self.assertEqual(ranked["result"]["records"][0]["node"]["id"], "a")
		self.assertEqual(ranked["result"]["records"][0]["match"],
			{"rank": 0, "reason": "exact-node-id"})

	def test_neighbors_should_return_bounded_multihop_parallel_evidence(self):
		one_hop = self.execute({"operation": "neighbors", "id": "a", "direction": "outgoing"})
		self.assertEqual(len(one_hop["result"]["records"]), 2)
		self.assertEqual({record["to"] for record in one_hop["result"]["records"]}, {"b"})
		self.assertEqual(len({record["evidence_sha256"] for record in one_hop["result"]["records"]}), 2)
		two_hop = self.execute({"operation": "neighbors", "id": "a", "direction": "outgoing", "depth": 2})
		self.assertIn("e", {record["to"] for record in two_hop["result"]["records"]})
		self.assertEqual(two_hop["result"]["metadata"]["nodes_reached"], 3)
		self.assertEqual(two_hop["result"]["metadata"]["activity_policy"],
			"exclude explicit inactive (including inherited owner/function false); retain missing as unclassified")

	def test_neighbors_should_emit_each_canonical_evidence_once_and_report_limits(self):
		payload = {"schema_version": 2, "nodes": [
			{"id": "a", "kind": "pallet"}, {"id": "b", "kind": "pallet"}], "edges": [
				{"source": "a", "target": "b", "kind": "call"},
			]}
		index = query_graph.GraphIndex(payload)
		records, matched, reasons, metadata = query_graph.neighbors(index,
			{"id": "a", "direction": "both", "depth": 2})
		self.assertEqual((len(records), matched, reasons), (1, 1, []))
		self.assertEqual((records[0]["edge"]["source"], records[0]["edge"]["target"]), ("a", "b"))

		loops = query_graph.GraphIndex({"schema_version": 2,
			"nodes": [{"id": "a", "kind": "pallet"}], "edges": [
				{"source": "a", "target": "a", "kind": "loop", "line": 1},
				{"source": "a", "target": "a", "kind": "loop", "line": 2},
			]})
		records, matched, reasons, _ = query_graph.neighbors(loops,
			{"id": "a", "direction": "both", "depth": 1})
		self.assertEqual((len(records), matched), (2, 2))
		self.assertEqual(len({record["evidence_sha256"] for record in records}), 2)
		self.assertEqual(reasons, [])
		records, matched, reasons, metadata = query_graph.neighbors(loops,
			{"id": "a", "direction": "both", "depth": 1, "max_expansions": 1})
		self.assertEqual((len(records), matched, metadata["expansions"]), (1, 1, 1))
		self.assertEqual(reasons, ["expansion-limit"])
		response = json.loads(query_graph.execute(loops, self.companions,
			{"operation": "neighbors", "id": "a", "direction": "both", "depth": 1,
				"max_expansions": 1}, 50, 4_000, self.fingerprint))
		self.assertFalse(response["result"]["total_is_exact"])
		self.assertIsNone(response["result"]["omitted"])
		minimal = query_graph.minimal_record(records[0])
		self.assertEqual({key: minimal[key] for key in ("depth", "from", "to")},
			{"depth": 1, "from": "a", "to": "a"})
		chain = query_graph.GraphIndex({"schema_version": 2, "nodes": [
			{"id": ident, "kind": "pallet"} for ident in ("a", "b", "c")], "edges": [
				{"source": "a", "target": "b", "kind": "call"},
				{"source": "b", "target": "c", "kind": "call"},
			]})
		_, _, reasons, metadata = query_graph.neighbors(chain,
			{"id": "a", "direction": "outgoing", "depth": 1})
		self.assertEqual(reasons, ["depth-limit"])
		self.assertFalse(metadata["search_complete"])

	def test_paths_should_preserve_parallel_evidence_and_report_search_limits(self):
		all_paths = self.execute({"operation": "paths", "source": "a", "target": "e",
			"max_depth": 2, "max_paths": 10})
		self.assertEqual(all_paths["result"]["matched"], 1)
		first_step = all_paths["result"]["records"][0]["steps"][0]
		self.assertEqual(first_step["evidence_count"], 2)
		self.assertEqual(len(set(first_step["evidence_sha256s"])), 2)
		limited = self.execute({"operation": "paths", "source": "a", "target": "e",
			"max_depth": 1, "max_paths": 1})
		self.assertTrue(limited["truncated"])
		self.assertIn("depth-limit", limited["truncation_reasons"])

	def test_paths_should_support_component_graph_view(self):
		components = query_graph.component_index(self.index, {"schema_version": 2,
			"projection": "execution", "edges": [
			{"source": "a", "target": "b", "kind": "component-call"},
			{"source": "a", "target": "b", "kind": "mir-component-call", "line": 4},
			{"source": "b", "target": "e", "kind": "component-call"},
		]})
		payload = json.loads(query_graph.execute(self.index, self.companions,
			{"operation": "paths", "source": "a", "target": "e", "view": "components"},
			50, 4_000, self.fingerprint, components))
		self.assertEqual(payload["result"]["matched"], 1)
		self.assertEqual(payload["result"]["metadata"]["view"], "components")
		step = payload["result"]["records"][0]["steps"][0]
		self.assertEqual(step["edge_kinds"], ["component-call", "mir-component-call"])
		self.assertEqual(step["evidence_count"], 2)
		self.assertEqual(len(step["evidence_variants"]), 2)
		self.assertNotIn("c", components.nodes)
		with self.assertRaisesRegex(query_graph.QueryFailure, "graph node does not exist"):
			query_graph.paths(components, {"source": "c", "target": "c"})

	def test_node_should_return_metadata_and_operational_edge_counts(self):
		payload = self.execute({"operation": "node", "id": "d"})
		record = payload["result"]["records"][0]
		self.assertEqual(record["runtime_activity"], "inactive")
		self.assertEqual(record["edge_counts"]["incoming"], 1)
		self.assertEqual(record["edge_counts"]["operational_incoming"], 0)

	def test_packs_should_list_and_filter_stored_query_sections(self):
		sections = self.execute({"operation": "packs"})
		self.assertEqual([record["section"] for record in sections["result"]["records"]],
			["cycles", "origins"])
		cycles = self.execute({"operation": "packs", "section": "cycles", "contains": "a"})
		self.assertEqual(cycles["result"]["matched"], 1)

	def test_output_should_obey_record_and_approximate_token_budgets(self):
		payload = self.execute({"operation": "search", "text": "a", "scope": "all"},
			max_records=2, max_tokens=800)
		self.assertLessEqual(payload["result"]["returned"], 2)
		self.assertIn("record-limit", payload["truncation_reasons"])
		self.assertLessEqual(payload["budget"]["estimated_tokens"], 800)
		self.assertEqual(payload["result"]["omitted"],
			payload["result"]["matched"] - payload["result"]["returned"])

	def test_tiny_budget_should_size_large_prefix_with_bounded_encoder_calls(self):
		records = [{"record_type": "node", "runtime_activity": "unclassified",
			"node": {"id": f"node:{index:05}", "kind": "function", "payload": "x" * 1_000}}
			for index in range(10_000)]
		original = query_graph.encode_envelope
		with mock.patch.object(query_graph, "encode_envelope", wraps=original) as encoder:
			text = query_graph.render_envelope("search", {"text": "node"}, records, len(records),
				[], [], 10_000, 256, graph_sha256=self.fingerprint)
		self.assertLessEqual(encoder.call_count, 32)
		self.assertLessEqual(len(text) + 1, 256 * 4)
		payload = json.loads(text)
		self.assertIn("token-limit", payload["truncation_reasons"])
		self.assertLessEqual(payload["budget"]["estimated_tokens"], 256)
		self.assertEqual(payload["result"]["omitted"],
			payload["result"]["matched"] - payload["result"]["returned"])

	def test_broad_search_should_complete_under_tiny_budget(self):
		index = query_graph.GraphIndex({"schema_version": 2,
			"nodes": [{"id": f"node:{number:05}", "kind": "function", "name": "match-node"}
				for number in range(2_000)], "edges": []})
		text = query_graph.execute(index, {"coverage": None, "completeness": None,
			"query_packs": None, "component_graph": None},
			{"operation": "search", "text": "node"}, 10_000, 256, self.fingerprint)
		payload = json.loads(text)
		self.assertEqual(payload["result"]["matched"], 2_000)
		self.assertLessEqual(len(text) + 1, 256 * 4)

	def test_invalid_graph_should_reject_dangling_edges(self):
		payload = fixture_graph()
		payload["edges"].append({"source": "a", "target": "missing", "kind": "call"})
		with self.assertRaisesRegex(query_graph.QueryFailure, "dangling graph edge"):
			query_graph.GraphIndex(payload)
		payload = fixture_graph()
		payload["schema_version"] = 1
		with self.assertRaisesRegex(query_graph.QueryFailure, "schema_version 2"):
			query_graph.GraphIndex(payload)

	def test_component_graph_schema_and_coverage_counts_should_be_validated(self):
		with self.assertRaisesRegex(query_graph.QueryFailure, "schema_version 2"):
			query_graph.component_index(self.index, {"projection": "execution", "edges": []})
		with self.assertRaisesRegex(query_graph.QueryFailure, "execution projection"):
			query_graph.component_index(self.index,
				{"schema_version": 2, "projection": "state", "edges": []})
		companions = fixture_companions()
		companions["coverage"].update({"nodes": 999, "edges": 998})
		warnings = query_graph.companion_warnings(companions, self.index)
		mismatches = [item for item in warnings if item["code"] == "companion-count-mismatch"]
		self.assertEqual({item["details"]["field"] for item in mismatches}, {"nodes", "edges"})

	def test_cli_batch_should_auto_load_companions_and_continue_after_errors(self):
		with tempfile.TemporaryDirectory() as directory:
			root = Path(directory)
			graph = root / "interaction-graph.json"
			graph.write_text(json.dumps(fixture_graph()))
			(root / "coverage.json").write_text(json.dumps(self.companions["coverage"]))
			(root / "completeness.json").write_text(json.dumps(self.companions["completeness"]))
			(root / "query-packs.json").write_text(json.dumps(self.companions["query_packs"]))
			(root / "component-graph.json").write_text(json.dumps({"schema_version": 2,
				"projection": "execution", "edges": [
					{"source": "a", "target": "b", "kind": "component-call"}]}))
			requests = "\n".join((
				json.dumps({"operation": [], "max_records": 1, "max_tokens": 256}),
				json.dumps({"operation": "summary"}),
				json.dumps({"operation": "search", "text": "bridge", "scope": []}),
				json.dumps({"operation": "search", "text": "bridge"}),
			)) + "\n"
			completed = subprocess.run([sys.executable, MODULE_PATH.as_posix(), "--graph", graph.as_posix(),
				"--max-tokens", "2000", "batch"], input=requests, text=True, capture_output=True, check=False)
			self.assertEqual(completed.returncode, 2)
			responses = [json.loads(line) for line in completed.stdout.splitlines()]
			self.assertEqual(completed.stderr, "")
			self.assertEqual(len(responses), 4)
			self.assertEqual(responses[0]["result"]["metadata"]["error"], "invalid-operation")
			self.assertEqual(responses[0]["budget"], {"max_records": 1, "max_tokens": 256,
				"estimation": "ceil(serialized ASCII characters / 4)",
				"estimated_tokens": responses[0]["budget"]["estimated_tokens"]})
			self.assertEqual(responses[1]["result"]["metadata"].get("error"), None)
			self.assertEqual(responses[2]["result"]["metadata"]["error"], "invalid-query")
			self.assertEqual(responses[3]["result"]["matched"], 1)
			self.assertEqual(responses[3]["graph"]["fingerprint"]["value"],
				hashlib.sha256(graph.read_bytes()).hexdigest())
			self.assertEqual(set(responses[3]["graph"]["companions"]),
				{"coverage", "completeness", "query_packs", "component_graph"})

	def test_published_response_schema_should_match_envelope_shape(self):
		schema = json.loads(MODULE_PATH.with_name("query-response.schema.json").read_text())
		self.assertEqual(schema["properties"]["schema_version"]["const"], 1)
		self.assertEqual(schema["properties"]["tool"]["const"], query_graph.TOOL)
		payload = self.execute({"operation": "summary"})
		self.assertEqual(set(schema["required"]), set(payload))


if __name__ == "__main__":
	unittest.main()
