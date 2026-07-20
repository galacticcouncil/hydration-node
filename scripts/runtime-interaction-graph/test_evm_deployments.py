import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock


DIRECTORY = Path(__file__).parent
CI_FIXTURE = DIRECTORY / "fixtures/ci-contracts.json"


def load(name: str):
	path = DIRECTORY / name
	spec = importlib.util.spec_from_file_location(path.stem, path)
	module = importlib.util.module_from_spec(spec)
	spec.loader.exec_module(module)
	return module


contracts = load("collect_contracts.py")
enrichment = load("enrich_contracts_rpc.py")
build = load("build_evm_graph.py")


GENESIS = "0x" + "11" * 32


def chain(network="hydration"):
	return {"id": "test", "evm_chain_id": 42, "substrate_genesis_hash": GENESIS,
		"substrate_spec_name": "hydradx", "runtime_configuration_minimums": {
			"total": 1, "asset_registry_erc20": 1, "required_bindings": {
				"gigaHdx.gigaHdxPoolContract": 1, "hsm.flashMinter": 1,
				"liquidation.borrowingContract": 1}}, "deployment_networks": [network]}


class ContractCollectionTests(unittest.TestCase):
	def test_nested_tuple_signatures_and_selectors_should_be_canonical(self):
		abi = json.loads((DIRECTORY / "fixtures/nested-tuple-abi.json").read_text())
		self.assertEqual(contracts.abi_functions(abi), [
			{"signature": "bar((address,(uint256,bytes32)[])[])", "selector": "0x306724ff"},
			{"signature": "transfer(address,uint256)", "selector": "0xa9059cbb"},
		])
		self.assertEqual(contracts.canonical_abi_type({"type": "uint[]"}), "uint256[]")
		self.assertEqual(contracts.canonical_abi_type({"type": "int[2][]"}), "int256[2][]")

	def test_collection_should_record_descriptor_source_and_artifact_hashes(self):
		with tempfile.TemporaryDirectory() as directory:
			root = Path(directory)
			hardhat = root / "hardhat/hydration"
			whm = root / "whm"
			hardhat.mkdir(parents=True)
			whm.mkdir()
			abi = json.loads((DIRECTORY / "fixtures/nested-tuple-abi.json").read_text())
			(hardhat / "Pool.json").write_text(json.dumps({
				"address": "0x1111111111111111111111111111111111111111", "abi": abi,
			}))
			(whm / "production.json").write_text(json.dumps({"steps": [{
				"name": "deploy", "status": "completed",
				"output": {
					"proxyAddress": "0x2222222222222222222222222222222222222222",
					"ownerAddress": "0x3333333333333333333333333333333333333333",
					"asset": "0x4444444444444444444444444444444444444444",
				},
			}]}))
			descriptor = root / "sources.json"
			descriptor.write_text(json.dumps({"schema_version": 2,
				"chains": [chain()],
				"sources": [
					{"project": "hardhat", "kind": "hardhat", "root": "hardhat", "networks": ["hydration"]},
					{"project": "whm", "kind": "whm", "root": "whm", "networks": ["production"]},
				]}))
			payload = contracts.collect(descriptor)
			self.assertEqual(payload["schema_version"], 2)
			self.assertEqual(len(payload["contracts"]), 2)
			self.assertEqual(payload["collection_provenance"]["descriptor_sha256"],
				contracts.sha256_file(descriptor))
			self.assertTrue(all(item["artifact_sha256"] for item in payload["contracts"]))
			pool = next(item for item in payload["contracts"] if item["project"] == "hardhat")
			self.assertEqual(pool["abi_functions"][0]["signature"], "bar((address,(uint256,bytes32)[])[])")
			whm_contract = next(item for item in payload["contracts"] if item["project"] == "whm")
			self.assertEqual(whm_contract["deployment_role"], "proxy")
			self.assertEqual({item["role"] for item in payload["address_references"]},
				{"authorization-account", "asset"})
			self.assertNotIn("0x3333333333333333333333333333333333333333",
				{item["address"] for item in payload["contracts"]})

	def test_collection_should_fail_when_required_source_is_missing(self):
		with tempfile.TemporaryDirectory() as directory:
			descriptor = Path(directory) / "sources.json"
			descriptor.write_text(json.dumps({"schema_version": 2,
				"chains": [chain()],
				"sources": [{"project": "missing", "kind": "hardhat", "root": "missing",
					"networks": ["hydration"]}]}))
			with self.assertRaises(FileNotFoundError):
				contracts.collect(descriptor)

	def test_descriptor_should_require_substrate_identity_in_schema_v2(self):
		with tempfile.TemporaryDirectory() as directory:
			descriptor = Path(directory) / "sources.json"
			descriptor.write_text(json.dumps({"schema_version": 1, "chains": [chain()], "sources": []}))
			with self.assertRaisesRegex(ValueError, "schema_version 2"):
				contracts.load_descriptor(descriptor)
			descriptor.write_text(json.dumps({"schema_version": 2,
				"chains": [{"id": "test", "evm_chain_id": 42, "deployment_networks": ["hydration"]}],
				"sources": []}))
			with self.assertRaisesRegex(ValueError, "chain descriptor"):
				contracts.load_descriptor(descriptor)

	def test_whm_references_should_have_typed_roles_without_becoming_contracts(self):
		with tempfile.TemporaryDirectory() as directory:
			root = Path(directory)
			(root / "production.json").write_text(json.dumps({"steps": [
				{"name": "001-deploy-router", "status": "completed", "output": {
					"implAddress": "0x1111111111111111111111111111111111111111",
					"proxyAddress": "0x2222222222222222222222222222222222222222",
					"ownerAddress": "0x3333333333333333333333333333333333333333"}},
				{"name": "002-configure", "status": "completed", "output": {
					"sourceAsset": "0x4444444444444444444444444444444444444444",
					"oracle": "0x5555555555555555555555555555555555555555",
					"handler": "0x6666666666666666666666666666666666666666",
					"proxyAddress": "0x7777777777777777777777777777777777777777"}},
			]}))
			items, references, _ = contracts.whm_deployments("whm", root, {"production"})
			self.assertEqual({item["deployment_role"] for item in items}, {"implementation", "proxy"})
			self.assertEqual({item["role"] for item in references},
				{"authorization-account", "asset", "oracle", "handler", "contract-reference"})
			self.assertTrue({item["address"] for item in items}.isdisjoint(
				{item["address"] for item in references}))


class RpcEnrichmentTests(unittest.TestCase):
	def payload(self):
		return {"schema_version": 2, "collection_provenance": {
			"descriptor_sha256": "a" * 64, "sources": [{"project": "p", "input_sha256": "b" * 64}]},
			"address_references": [{"project": "p", "network": "hydration", "migration_step": "configure",
				"field": "asset", "role": "asset", "address": "0x3333333333333333333333333333333333333333",
				"artifact_sha256": "e" * 64}],
			"contracts": [
				{"project": "p", "network": "hydration", "name": "Pool",
					"address": "0x1111111111111111111111111111111111111111", "artifact_sha256": "c" * 64},
				{"project": "p", "network": "hydration", "name": "Token",
					"address": "0x2222222222222222222222222222222222222222", "artifact_sha256": "d" * 64},
			]}

	def rpc_call(self, _url, method, _params):
		return {"eth_chainId": "0x2a", "eth_getBlockByNumber": {
			"number": "0x10", "hash": "0xabc", "parentHash": "0xdef", "stateRoot": "0x123",
		}}[method]

	def test_enrichment_should_record_chain_address_and_input_provenance(self):
		implementation = "3333333333333333333333333333333333333333"
		results = ["0x6000" + "22" * 20, "0x" + "00" * 32,
			"0x6001", "0x" + "00" * 12 + implementation]
		result = enrichment.enrich(self.payload(), "rpc", "0x10", {"hydration"}, 42, "input-hash",
			rpc_call=self.rpc_call, rpc_batch_call=lambda _url, _calls: results)
		self.assertEqual(result["rpc_snapshot"]["chain_id"], 42)
		self.assertEqual(result["enrichment_provenance"]["input_sha256"], "input-hash")
		self.assertEqual(result["observations"][0]["chain_address_id"],
			"eip155:42:0x1111111111111111111111111111111111111111")
		self.assertEqual(result["observations"][0]["embedded_addresses"],
			["0x2222222222222222222222222222222222222222"])
		self.assertEqual(result["observations"][1]["implementation_chain_address_id"],
			f"eip155:42:0x{implementation}")
		self.assertEqual(result["address_references"], self.payload()["address_references"])
		self.assertNotIn(self.payload()["address_references"][0]["address"],
			{item["address"] for item in result["observations"]})

	def test_enrichment_should_reject_wrong_rpc_chain(self):
		with self.assertRaisesRegex(ValueError, "does not match expected"):
			enrichment.enrich(self.payload(), "rpc", "0x10", {"hydration"}, 43, "input-hash",
				rpc_call=self.rpc_call, rpc_batch_call=lambda _url, _calls: [])

	def test_enrichment_should_reject_untyped_address_reference(self):
		payload = self.payload()
		payload["address_references"][0].pop("role")
		with self.assertRaisesRegex(ValueError, "address reference inventory"):
			enrichment.validate_collection(payload)


class EvmBuildTests(unittest.TestCase):
	def test_ci_fixture_should_satisfy_runtime_configuration_minimums(self):
		payload = json.loads(CI_FIXTURE.read_text())
		chain_config = build.load_chain(CI_FIXTURE, "hydration-mainnet-ci")
		self.assertEqual(build.validate_runtime_configuration_coverage(
			payload, chain_config["runtime_configuration_minimums"]), {
			"total": 4,
			"asset_registry_erc20": 1,
			"required_bindings": {
				"gigaHdx.gigaHdxPoolContract": 1,
				"hsm.flashMinter": 1,
				"liquidation.borrowingContract": 1,
			},
			"required_queries": 5,
		})

	def test_output_hashes_should_cover_nested_outputs_and_exclude_self(self):
		with tempfile.TemporaryDirectory() as directory:
			output = Path(directory)
			(output / "focused").mkdir()
			(output / "graph.json").write_text("graph")
			(output / "focused/view.svg").write_text("svg")
			(output / "evm-build-provenance.json").write_text("old")
			self.assertEqual(set(build.output_hashes(output)), {"graph.json", "focused/view.svg"})

	def test_load_chain_should_require_expected_substrate_identity(self):
		with tempfile.TemporaryDirectory() as directory:
			descriptor = Path(directory) / "sources.json"
			descriptor.write_text(json.dumps({"schema_version": 2, "chains": [chain()]}))
			self.assertEqual(build.load_chain(descriptor, "test")["substrate_genesis_hash"], GENESIS)
			descriptor.write_text(json.dumps({"schema_version": 2, "chains": [
				{"id": "test", "evm_chain_id": 42, "deployment_networks": ["hydration"]}]}))
			with self.assertRaisesRegex(ValueError, "invalid chain descriptor"):
				build.load_chain(descriptor, "test")

	def test_runtime_configuration_coverage_should_enforce_descriptor_minimums(self):
		payload = {"runtime_collection_provenance": {"query_coverage": {
			"required_query_count": 5, "available_query_count": 5}}, "runtime_configurations": [
			{"component": "pallet:gigahdx", "storage": "gigaHdx.gigaHdxPoolContract"},
			{"component": "pallet:hsm", "storage": "hsm.flashMinter"},
			{"component": "pallet:liquidation", "storage": "liquidation.borrowingContract"},
			{"component": "pallet:asset-registry", "storage": "assetRegistry.assetLocations",
				"asset_type": "erc20"},
		]}
		minimums = {"total": 4, "asset_registry_erc20": 1, "required_bindings": {
			"gigaHdx.gigaHdxPoolContract": 1, "hsm.flashMinter": 1,
			"liquidation.borrowingContract": 1}}
		self.assertEqual(build.validate_runtime_configuration_coverage(
			payload, minimums),
			{"total": 4, "asset_registry_erc20": 1, "required_bindings": {
				"gigaHdx.gigaHdxPoolContract": 1, "hsm.flashMinter": 1,
				"liquidation.borrowingContract": 1}, "required_queries": 5})
		with self.assertRaisesRegex(ValueError, "asset_registry_erc20=1 is below minimum 2"):
			build.validate_runtime_configuration_coverage(payload,
				{**minimums, "asset_registry_erc20": 2})
		payload["runtime_configurations"] = payload["runtime_configurations"][1:]
		with self.assertRaisesRegex(ValueError, "gigaHdx.gigaHdxPoolContract=0 is below minimum 1"):
			build.validate_runtime_configuration_coverage(payload,
				{**minimums, "total": 3})
		payload["runtime_collection_provenance"]["query_coverage"]["available_query_count"] = 4
		with self.assertRaisesRegex(ValueError, "incomplete required-query coverage"):
			build.validate_runtime_configuration_coverage(payload, minimums)

	def test_graph_command_should_only_include_explicit_semantic_manifests(self):
		command = build.graph_command(Path("scripts"), Path("runtime.json"), Path("output"), None, None)
		self.assertNotIn("--rapx-manifest", command)
		self.assertNotIn("--rustc-mir-manifest", command)
		self.assertEqual(command[command.index("--coverage-thresholds") + 1],
			Path("scripts/coverage-thresholds.json"))
		command = build.graph_command(Path("scripts"), Path("runtime.json"), Path("output"),
			Path("rapx.json"), Path("mir.json"))
		self.assertIn("--rapx-manifest", command)
		self.assertIn("--rustc-mir-manifest", command)
		self.assertEqual(command[command.index("--coverage-thresholds") + 1],
			Path("scripts/coverage-thresholds-full.json"))

	def test_semantic_manifest_should_reuse_strict_validator_and_return_logical_paths(self):
		with tempfile.TemporaryDirectory() as directory:
			root = Path(directory)
			manifest = root / "semantic/manifest.json"
			manifest.parent.mkdir()
			source_inputs = {"sha256": "a" * 64, "file_count": 1}
			payload = {"schema_version": 2, "tool": "rapx", "toolchain": "nightly",
				"provenance": {"source_inputs": source_inputs, "collector_sha256": "b" * 64},
				"packages": [{"package": "pallet-example", "analyses": {
					"callgraph": {"status": "ok", "path": "artifacts/callgraph.txt",
						"artifact_sha256": "c" * 64}}}]}
			manifest.write_text(json.dumps(payload))
			with mock.patch.object(build.runtime_graph, "validate_semantic_manifest") as validator:
				result = build.validate_semantic_manifest(manifest, "rapx", source_inputs, root)
			validator.assert_called_once_with(payload, manifest, "rapx", root)
			self.assertEqual(result["path"], "semantic/manifest.json")
			self.assertFalse(Path(result["path"]).is_absolute())
			self.assertEqual(result["artifacts"], [
				{"path": "artifacts/callgraph.txt", "sha256": "c" * 64},
			])
			self.assertFalse(Path(result["artifacts"][0]["path"]).is_absolute())
			with self.assertRaisesRegex(ValueError, "stale or different"):
				with mock.patch.object(build.runtime_graph, "validate_semantic_manifest"):
					build.validate_semantic_manifest(
						manifest, "rapx", {"sha256": "d" * 64, "file_count": 1}, root)

	def test_graph_generator_fingerprint_should_cover_transitive_inputs_and_active_policy(self):
		with tempfile.TemporaryDirectory() as directory:
			scripts = Path(directory)
			for name in (*build.GRAPH_GENERATOR_INPUTS, "coverage-thresholds.json",
				"coverage-thresholds-full.json"):
				(scripts / name).write_text(name)
			standard = build.graph_generator_fingerprint(scripts, None)
			self.assertEqual(standard, build.graph_generator_fingerprint(scripts, None))
			self.assertTrue({"graph_explorer.js", "graph_explorer.css"}.issubset(
				build.GRAPH_GENERATOR_INPUTS))
			self.assertEqual(set(standard["files"]),
				{*build.GRAPH_GENERATOR_INPUTS, "coverage-thresholds.json"})
			self.assertNotIn("coverage-thresholds-full.json", standard["files"])
			full = build.graph_generator_fingerprint(scripts, Path("mir.json"))
			self.assertEqual(set(full["files"]),
				{*build.GRAPH_GENERATOR_INPUTS, "coverage-thresholds-full.json"})
			self.assertNotEqual(standard["sha256"], full["sha256"])
			(scripts / "mir_parser.py").write_text("changed")
			self.assertNotEqual(full["sha256"],
				build.graph_generator_fingerprint(scripts, Path("mir.json"))["sha256"])

	def test_prepare_output_should_reject_non_empty_directory_before_build(self):
		with tempfile.TemporaryDirectory() as directory:
			output = Path(directory) / "new"
			build.prepare_output(output)
			self.assertTrue(output.is_dir())
			build.prepare_output(output)
			(output / "stale.json").write_text("stale")
			with self.assertRaisesRegex(ValueError, "must be empty"):
				build.prepare_output(output)


if __name__ == "__main__":
	unittest.main()
