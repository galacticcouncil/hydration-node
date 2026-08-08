import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


MODULE = Path(__file__).with_name("runtime_interaction_graph.py")
CI_FIXTURE = Path(__file__).with_name("fixtures") / "ci-contracts.json"
SPEC = importlib.util.spec_from_file_location("runtime_interaction_graph_contracts", MODULE)
graph = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(graph)


CHAIN_A = 42
CHAIN_B = 43
PROXY = "0x1111111111111111111111111111111111111111"
IMPLEMENTATION = "0x2222222222222222222222222222222222222222"
UNOBSERVED = "0x3333333333333333333333333333333333333333"


def chain_address(chain_id, address):
	return f"eip155:{chain_id}:{address}"


def observation(project, network, chain_id, address, **data):
	return {"project": project, "network": network, "chain_id": chain_id, "address": address,
		"chain_address_id": chain_address(chain_id, address), "has_code": True, **data}


class ContractGraphImportTests(unittest.TestCase):
	def import_payload(self, payload):
		with tempfile.TemporaryDirectory() as directory:
			path = Path(directory) / "contracts.json"
			path.write_text(json.dumps(payload))
			result = graph.Graph()
			count = graph.merge_contracts(result, path)
			return result, count

	def test_schema_v2_uses_chain_physical_identity_and_separate_aliases(self):
		contracts = [
			{"project": "aave", "network": "hydration", "name": "Pool-Proxy", "address": PROXY,
				"artifact": "aave/pool.json", "abi_functions": [
					{"signature": "supply(address,uint256)", "selector": "0xf2b9fdb8"}],
				"abi_signatures": ["supply(address,uint256)"]},
			{"project": "hollar", "network": "hydration", "name": "HollarPool", "address": PROXY,
				"artifact": "hollar/pool.json", "abi_functions": [
					{"signature": "supply(address,uint256)", "selector": "0xf2b9fdb8"}]},
			{"project": "aave", "network": "hydration", "name": "Pool-Implementation",
				"address": IMPLEMENTATION, "artifact": "aave/implementation.json", "abi_functions": []},
			{"project": "other", "network": "other-chain", "name": "SameRawAddress",
				"address": IMPLEMENTATION, "artifact": "other/contract.json", "abi_functions": []},
			{"project": "whm", "network": "external", "name": "Router", "address": UNOBSERVED,
				"artifact": "whm/router.json", "abi_functions": []},
		]
		observations = [
			observation("aave", "hydration", CHAIN_A, PROXY, implementation=IMPLEMENTATION,
				implementation_chain_address_id=chain_address(CHAIN_A, IMPLEMENTATION),
				embedded_addresses=[IMPLEMENTATION]),
			observation("hollar", "hydration", CHAIN_A, PROXY, implementation=IMPLEMENTATION,
				implementation_chain_address_id=chain_address(CHAIN_A, IMPLEMENTATION),
				embedded_addresses=[IMPLEMENTATION]),
			observation("aave", "hydration", CHAIN_A, IMPLEMENTATION, implementation=None,
				implementation_chain_address_id=None, embedded_addresses=[]),
			observation("other", "other-chain", CHAIN_B, IMPLEMENTATION, implementation=None,
				implementation_chain_address_id=None, embedded_addresses=[]),
		]
		payload = {"schema_version": 2, "contracts": contracts, "observations": observations,
			"address_references": [{"project": "whm", "network": "external",
				"migration_step": "configure-token", "field": "sourceAsset", "role": "asset",
				"address": "0x4444444444444444444444444444444444444444",
				"artifact": "whm/external.json", "artifact_sha256": "reference-hash"}],
			"rpc_snapshot": {"chain_id": CHAIN_A, "block_hash": "0xevm", "block_number": 10},
			"substrate_snapshot": {"genesis_hash": "0xgenesis", "block_hash": "0xsubstrate",
				"block_number": 10},
			"chain_context": {"evm_chain_id": CHAIN_A, "evm_block_hash": "0xevm",
				"substrate_genesis_hash": "0xgenesis", "substrate_block_hash": "0xsubstrate",
				"block_number": 10},
			"collection_provenance": {"descriptor_sha256": "descriptor"},
			"enrichment_provenance": {"input_sha256": "collection"},
			"runtime_collection_provenance": {"input_sha256": "enrichment"},
			"runtime_configurations": [{"component": "pallet:hsm", "storage": "hsm.flashMinter",
				"chain_id": CHAIN_B, "address": IMPLEMENTATION,
				"chain_address_id": chain_address(CHAIN_B, IMPLEMENTATION)}]}
		result, count = self.import_payload(payload)

		proxy = f"deployed-contract:{chain_address(CHAIN_A, PROXY)}"
		implementation_a = f"deployed-contract:{chain_address(CHAIN_A, IMPLEMENTATION)}"
		implementation_b = f"deployed-contract:{chain_address(CHAIN_B, IMPLEMENTATION)}"
		self.assertEqual(count, 3)
		self.assertIn(proxy, result.nodes)
		self.assertIn(implementation_a, result.nodes)
		self.assertIn(implementation_b, result.nodes)
		self.assertIn(f"deployment-alias:aave:hydration:{PROXY}", result.nodes)
		self.assertIn(f"deployment-alias:hollar:hydration:{PROXY}", result.nodes)
		self.assertIn(f"deployment-alias:whm:external:{UNOBSERVED}", result.nodes)
		self.assertNotIn(f"deployed-contract:{chain_address(CHAIN_A, UNOBSERVED)}", result.nodes)
		reference = "deployment-address-reference:whm:external:0x4444444444444444444444444444444444444444"
		self.assertEqual(result.nodes[reference]["kind"], "deployment-address-reference")
		self.assertFalse(result.nodes[reference]["onchain_observed"])
		self.assertTrue(any(edge["source"] == "deployment-step:whm:external:configure-token"
			and edge["target"] == reference and edge["kind"] == "deployment-step-references-address"
			and edge["role"] == "asset" for edge in result.edges))
		self.assertFalse(any(node["kind"] == "deployed-contract" and
			node.get("address") == "0x4444444444444444444444444444444444444444"
			for node in result.nodes.values()))

		proxy_edges = [edge for edge in result.edges
			if edge["source"] == proxy and edge["kind"] == "proxy-implementation"]
		self.assertEqual({edge["target"] for edge in proxy_edges}, {implementation_a})
		embedded_edges = [edge for edge in result.edges
			if edge["source"] == proxy and edge["kind"] == "bytecode-embeds-address"]
		self.assertEqual({edge["target"] for edge in embedded_edges}, {implementation_a})
		self.assertTrue(any(edge["source"] == "pallet:hsm" and edge["target"] == implementation_b
			and edge["kind"] == "runtime-configures-contract" for edge in result.edges))

		function = f"contract-function:{chain_address(CHAIN_A, PROXY)}:supply(address,uint256)"
		self.assertEqual(result.nodes[function]["selector"], "0xf2b9fdb8")
		self.assertEqual(result.nodes["evm-selector:0xf2b9fdb8"]["selector"], "0xf2b9fdb8")
		self.assertTrue(any(edge["source"] == "evm-selector:0xf2b9fdb8"
			and edge["target"] == function and edge["kind"] == "selector-matches-contract-function"
			for edge in result.edges))

		coverage = result.nodes["semantic-analysis:contract-deployments"]
		self.assertEqual(coverage["chain_context"], payload["chain_context"])
		self.assertEqual(coverage["rpc_snapshot"], payload["rpc_snapshot"])
		self.assertEqual(coverage["substrate_snapshot"], payload["substrate_snapshot"])
		self.assertEqual(coverage["collection_provenance"], payload["collection_provenance"])
		self.assertEqual(coverage["enrichment_provenance"], payload["enrichment_provenance"])
		self.assertEqual(coverage["runtime_collection_provenance"],
			payload["runtime_collection_provenance"])
		self.assertEqual(coverage["address_references"], 1)

	def test_ci_fixture_should_cover_deployments_and_required_runtime_bindings(self):
		payload = json.loads(CI_FIXTURE.read_text())
		result, count = self.import_payload(payload)
		self.assertEqual(count, 6)
		coverage = result.nodes["semantic-analysis:contract-deployments"]
		self.assertEqual(coverage["runtime_configurations"], 4)
		self.assertEqual(coverage["asset_registry_erc20_configurations"], 1)
		bindings = {(edge["source"], edge.get("storage")) for edge in result.edges
			if edge["kind"] == "runtime-configures-contract"}
		self.assertTrue({
			("pallet:gigahdx", "gigaHdx.gigaHdxPoolContract"),
			("pallet:hsm", "hsm.flashMinter"),
			("pallet:liquidation", "liquidation.borrowingContract"),
			("pallet:asset-registry", "assetRegistry.assetLocations"),
		}.issubset(bindings))
		self.assertTrue(any(edge["kind"] == "proxy-implementation" for edge in result.edges))
		self.assertTrue(any(edge["kind"] == "selector-matches-contract-function" for edge in result.edges))

	def test_selector_collisions_should_converge_on_physical_selector_identity(self):
		result = graph.Graph()
		_, first = graph.ensure_evm_selector(result, "burn(uint256)")
		_, second = graph.ensure_evm_selector(result, "collate_propagate_storage(bytes16)")
		self.assertEqual(first, "evm-selector:0x42966c68")
		self.assertEqual(first, second)
		self.assertEqual(result.nodes[first]["signatures"],
			["burn(uint256)", "collate_propagate_storage(bytes16)"])
		self.assertTrue(result.nodes[first]["selector_collision"])

	def test_schema_v2_rejects_cross_chain_proxy_resolution(self):
		payload = {"schema_version": 2, "contracts": [{"project": "p", "network": "n", "name": "Proxy",
			"address": PROXY, "artifact": "proxy.json", "abi_functions": []}], "observations": [
			observation("p", "n", CHAIN_A, PROXY, implementation=IMPLEMENTATION,
				implementation_chain_address_id=chain_address(CHAIN_B, IMPLEMENTATION), embedded_addresses=[])]}
		with self.assertRaisesRegex(ValueError, "same chain"):
			self.import_payload(payload)

	def test_schema_v2_chain_descriptor_canonicalizes_unobserved_deployments(self):
		payload = {"schema_version": 2, "chains": [{"id": "mainnet", "evm_chain_id": CHAIN_A,
			"deployment_networks": ["hydration"]}], "contracts": [{"project": "p", "network": "hydration",
				"name": "Pool", "address": PROXY, "artifact": "pool.json", "abi_functions": [
				{"signature": "supply(address,uint256)", "selector": "0xf2b9fdb8"}]}]}
		result, count = self.import_payload(payload)
		physical = f"deployed-contract:{chain_address(CHAIN_A, PROXY)}"
		self.assertEqual(count, 1)
		self.assertFalse(result.nodes[physical]["onchain_observed"])
		self.assertEqual(result.nodes[physical]["chain_address_id"], chain_address(CHAIN_A, PROXY))
		self.assertIn(f"contract-function:{chain_address(CHAIN_A, PROXY)}:supply(address,uint256)", result.nodes)

	def test_schema_v2_rejects_conflicting_canonical_selectors(self):
		payload = {"schema_version": 2, "contracts": [
			{"project": "a", "network": "n", "name": "A", "address": PROXY, "artifact": "a.json",
				"abi_functions": [{"signature": "f(uint256)", "selector": "0x11111111"}]},
			{"project": "b", "network": "n", "name": "B", "address": IMPLEMENTATION,
				"artifact": "b.json",
				"abi_functions": [{"signature": "f(uint256)", "selector": "0x22222222"}]},
		]}
		with self.assertRaisesRegex(ValueError, "conflicting selectors"):
			self.import_payload(payload)

	def test_unknown_manifest_schema_is_not_imported_as_legacy(self):
		with self.assertRaisesRegex(ValueError, "unsupported contract manifest schema_version"):
			self.import_payload({"schema_version": 3, "contracts": []})


if __name__ == "__main__":
	unittest.main()
