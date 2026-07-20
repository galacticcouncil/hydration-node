import importlib.util
import json
import unittest
from pathlib import Path


MODULE = Path(__file__).with_name("runtime_interaction_graph.py")
SPEC = importlib.util.spec_from_file_location("runtime_interaction_graph_correctness", MODULE)
graph = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(graph)
ROOT = MODULE.parents[2]


class GraphCorrectnessTests(unittest.TestCase):
	def test_runtime_mod_owners_should_include_their_relative_path(self):
		root = Path("/repo")
		paths = [
			root / "runtime/hydradx/src/xcm/mod.rs",
			root / "runtime/hydradx/src/governance/mod.rs",
			root / "runtime/hydradx/src/evm/accounts/mod.rs",
			root / "runtime/hydradx/src/evm/adapters/mod.rs",
			root / "runtime/hydradx/src/evm/precompiles/assets/mod.rs",
			root / "runtime/hydradx/src/evm/precompiles/dispatch/mod.rs",
		]
		owners = [graph.owner(path, root)[0] for path in paths]
		self.assertEqual(len(set(owners)), len(owners), dict(zip(map(str, paths), owners)))

	def test_function_source_ids_should_ignore_blank_lines_and_include_scope(self):
		source = """mod first {
	impl Handler for Alpha {
		fn execute() { alpha(); }
	}
	impl Handler for Beta {
		fn execute() { beta(); }
	}
}
mod second {
	impl Handler for Alpha {
		fn execute() { second(); }
	}
}
"""
		with_blank_lines = "\n\n" + source.replace("fn execute() {", "fn execute()\n\n\t\t{")

		def identifiers(text: str) -> list[str]:
			scopes = graph.scope_ranges(text)
			return [graph.function_source_id(text, match, "runtime/example.rs", scopes)
				for match in graph.FN.finditer(text)]

		original = identifiers(source)
		shifted = identifiers(with_blank_lines)
		self.assertEqual(original, shifted)
		self.assertEqual(len(original), 3)
		self.assertEqual(len(set(original)), len(original))

	def test_execution_projection_should_exclude_configuration_and_deployment_edges(self):
		g = graph.Graph()
		g.edge("pallet:a", "pallet:b", "direct-call")
		g.edge("pallet:b", "pallet:a", "config-binding")
		g.edge("pallet:c", "deployed-contract:x", "uses-deployed-contract")
		g.edge("deployed-contract:x", "boundary:evm-execution", "direct-call")

		self.assertIn(["pallet:a", "pallet:b"], graph.strongly_connected(g.edges))
		self.assertIn(
			["pallet:c", "deployed-contract:x", "boundary:evm-execution"],
			graph.bounded_paths(g.edges, {"boundary:evm-execution"}),
		)

		execution = graph.projected_edges(g, "execution")
		self.assertEqual({edge["kind"] for edge in execution}, {"direct-call"})
		self.assertEqual(graph.strongly_connected(execution), [])
		self.assertNotIn(
			["pallet:c", "deployed-contract:x", "boundary:evm-execution"],
			graph.bounded_paths(execution, {"boundary:evm-execution"}),
		)

	def test_real_scan_should_have_integral_unique_edges(self):
		g = graph.scan(ROOT)
		dangling = [edge for edge in g.edges if edge["source"] not in g.nodes or edge["target"] not in g.nodes]
		keys = [json.dumps(edge, sort_keys=True, separators=(",", ":"), default=str) for edge in g.edges]
		self.assertEqual(dangling, [])
		self.assertEqual(len(keys), len(set(keys)))


if __name__ == "__main__":
	unittest.main()
