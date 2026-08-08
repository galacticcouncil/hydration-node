import copy
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE = Path(__file__).with_name("semantic_inventory.py")
INVENTORY = Path(__file__).with_name("semantic-inventory.json")
SCHEMA = Path(__file__).with_name("semantic-inventory.schema.json")
SPEC = importlib.util.spec_from_file_location("semantic_inventory", MODULE)
semantic_inventory = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(semantic_inventory)


class SemanticInventoryTests(unittest.TestCase):
	@classmethod
	def setUpClass(cls):
		cls.payload = json.loads(INVENTORY.read_text(encoding="utf-8"))

	def test_inventory_should_validate_against_local_rust_sources(self):
		result = semantic_inventory.validate_inventory(copy.deepcopy(self.payload), ROOT)
		self.assertGreaterEqual(result["coverage"]["node_count"], 30)
		self.assertGreaterEqual(result["coverage"]["edge_count"], 35)
		self.assertEqual(result["coverage"]["domains"], [
			"asset-routing",
			"circuit-breaker",
			"omnipool",
			"stableswap",
			"xyk",
		])
		for item in result["nodes"] + result["edges"]:
			self.assertEqual(item["semantic_source"], "explicit-inventory")
			for evidence in item["evidence"]:
				self.assertGreater(evidence["line"], 0)
				self.assertRegex(evidence["source_sha256"], r"^[0-9a-f]{64}$")

	def test_inventory_should_cover_requested_ledger_and_pool_semantics(self):
		result = semantic_inventory.validate_inventory(copy.deepcopy(self.payload), ROOT)
		edges = {(edge["source"], edge["kind"], edge["target"]) for edge in result["edges"]}
		required = {
			("router:currencies", "routes-to", "ledger:native-balances"),
			("router:currencies", "routes-to", "ledger:orml-tokens"),
			("router:currencies", "routes-to", "ledger:erc20-contracts"),
			("operation:stableswap-add-liquidity", "mints", "ledger:stableswap-shares"),
			("operation:stableswap-remove-liquidity", "burns", "ledger:stableswap-shares"),
			("operation:stableswap-add-liquidity", "enforces", "invariant:stableswap-add-liquidity"),
			("operation:stableswap-remove-liquidity", "enforces", "invariant:stableswap-remove-liquidity"),
			("operation:stableswap-add-liquidity", "reads", "state:stableswap-share-issuance"),
			("operation:stableswap-remove-liquidity", "reads", "state:stableswap-share-issuance"),
			("state:stableswap-share-issuance", "tracks", "ledger:stableswap-shares"),
			("invariant:stableswap-share-issuance-sync", "reads", "state:stableswap-share-issuance"),
			("invariant:stableswap-share-issuance-sync", "reads", "ledger:stableswap-shares"),
			("state:xyk-total-liquidity", "must-equal", "ledger:xyk-shares"),
			("ledger:xyk-reserves", "backs", "ledger:xyk-shares"),
			("state:omnipool-positions", "must-equal", "state:omnipool-lp-shares"),
			("state:omnipool-assets", "must-equal", "ledger:omnipool-hub-reserve"),
			("guard:issuance-increase-fuse", "reads", "router:routed-total-issuance"),
			("operation:orml-token-deposit", "invokes", "guard:issuance-increase-fuse"),
		}
		self.assertTrue(required.issubset(edges), required - edges)

	def test_schema_should_match_loader_enumerations(self):
		schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
		self.assertEqual(set(schema["$defs"]["node"]["properties"]["kind"]["enum"]),
			set(semantic_inventory.ALLOWED_NODE_KINDS))
		self.assertEqual(set(schema["$defs"]["node"]["properties"]["domain"]["enum"]),
			set(semantic_inventory.ALLOWED_DOMAINS))
		self.assertEqual(set(schema["$defs"]["edge"]["properties"]["kind"]["enum"]),
			set(semantic_inventory.ALLOWED_EDGE_KINDS))
		self.assertEqual(set(schema["$defs"]["edge"]["properties"]["enforcement"]["enum"]),
			set(semantic_inventory.ALLOWED_ENFORCEMENT))

	def test_validation_should_fail_when_evidence_symbol_is_missing(self):
		payload = copy.deepcopy(self.payload)
		payload["nodes"][0]["evidence"][0]["symbol"] = "symbol that cannot exist"
		with self.assertRaisesRegex(semantic_inventory.InventoryError, "symbol was not found"):
			semantic_inventory.validate_inventory(payload, ROOT)

	def test_validation_should_fail_when_node_id_is_duplicated(self):
		payload = copy.deepcopy(self.payload)
		payload["nodes"].append(copy.deepcopy(payload["nodes"][0]))
		with self.assertRaisesRegex(semantic_inventory.InventoryError, "duplicate node id"):
			semantic_inventory.validate_inventory(payload, ROOT)

	def test_validation_should_fail_when_edge_is_dangling(self):
		payload = copy.deepcopy(self.payload)
		payload["edges"][0]["target"] = "ledger:missing"
		with self.assertRaisesRegex(semantic_inventory.InventoryError, "unknown node"):
			semantic_inventory.validate_inventory(payload, ROOT)

	def test_validation_should_fail_when_evidence_traverses_root(self):
		payload = copy.deepcopy(self.payload)
		payload["nodes"][0]["evidence"][0]["file"] = "../outside.rs"
		with self.assertRaisesRegex(semantic_inventory.InventoryError, "cannot traverse"):
			semantic_inventory.validate_inventory(payload, ROOT)

	def test_cli_should_write_normalized_inventory(self):
		with tempfile.TemporaryDirectory() as directory:
			output = Path(directory) / "semantic.json"
			subprocess.run([
				sys.executable,
				str(MODULE),
				"--root",
				str(ROOT),
				"--inventory",
				str(INVENTORY),
				"--output",
				str(output),
			], check=True, capture_output=True, text=True)
			written = json.loads(output.read_text(encoding="utf-8"))
			self.assertEqual(written["coverage"]["node_count"], len(written["nodes"]))


if __name__ == "__main__":
	unittest.main()
