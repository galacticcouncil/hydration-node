import importlib.util
import tempfile
import unittest
from pathlib import Path


MODULE = Path(__file__).with_name("runtime_inventory.py")
SPEC = importlib.util.spec_from_file_location("runtime_inventory", MODULE)
inventory = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(inventory)


class RuntimeInventoryTests(unittest.TestCase):
	def test_construct_runtime_entries_should_not_cross_lines(self):
		text = """
		EVM: pallet_evm = 90,
		Ethereum: pallet_ethereum = 92,
		TechnicalCommittee: pallet_collective::<Instance2> exclude_parts { Config } = 25,
		"""
		self.assertEqual(inventory.construct_runtime_entries(text), [
			{"alias": "EVM", "crate": "pallet_evm", "instance": None, "index": 90, "excluded_parts": []},
			{"alias": "Ethereum", "crate": "pallet_ethereum", "instance": None, "index": 92,
				"excluded_parts": []},
			{"alias": "TechnicalCommittee", "crate": "pallet_collective", "instance": "Instance2",
				"index": 25, "excluded_parts": ["Config"]},
		])

	def test_precompile_inventory_should_include_static_and_dynamic_routes(self):
		text = """
		pub const ECRECOVER: H160 = H160(hex!(\"0000000000000000000000000000000000000001\"));
		pub const DISPATCH_ADDR: H160 = addr(1025);
		if address == ECRECOVER { Some(ECRecover::execute(handle))
		} else if address == DISPATCH_ADDR { Some(Dispatch::<R>::execute(handle))
		} else if is_asset_address(address) { Some(MultiCurrencyPrecompile::<R>::execute(handle))
		} else if is_oracle_address(address) { Some(ChainlinkOraclePrecompile::<R>::execute(handle))
		}
		"""
		routes = inventory.precompile_inventory(text)
		by_route = {route["route"]: route for route in routes}
		self.assertEqual(by_route["ecrecover"]["address"], "0x" + "0" * 39 + "1")
		self.assertEqual(by_route["dispatch-addr"]["address"], "0x" + "0" * 36 + "0401")
		self.assertEqual(by_route["asset-address"]["predicate"], "is_asset_address")
		self.assertTrue(by_route["oracle-address"]["dynamic"])

	def test_active_external_sources_should_follow_file_and_module_cfgs(self):
		with tempfile.TemporaryDirectory() as directory:
			root = Path(directory)
			(root / "lib.rs").write_text('''
mod live;
mod integration_test;
#[cfg(any(test, feature = "test-utils"))]
pub mod xcm_helpers;
#[cfg(feature = "enabled")]
mod enabled;
#[cfg(not(feature = "std"))]
mod wasm;
''')
			(root / "live.rs").write_text("pub fn live() {}\n")
			(root / "integration_test.rs").write_text("#![cfg(test)]\npub fn test_only() {}\n")
			(root / "xcm_helpers.rs").write_text("mod nested;\npub fn helper() {}\n")
			(root / "xcm_helpers").mkdir()
			(root / "xcm_helpers/nested.rs").write_text("pub fn nested() {}\n")
			(root / "enabled.rs").write_text("pub fn enabled() {}\n")
			(root / "wasm.rs").write_text("pub fn wasm() {}\n")
			paths = {path.name for path in inventory.active_external_sources(root)}
			self.assertEqual(paths, {"lib.rs", "live.rs", "enabled.rs", "wasm.rs"})

	def test_cfg_value_should_retain_unknown_target_conditions_conservatively(self):
		self.assertFalse(inventory.cfg_value('any(test, feature = "test-utils")'))
		self.assertTrue(inventory.cfg_value('not(feature = "runtime-benchmarks")'))
		self.assertIsNone(inventory.cfg_value('feature = "std"'))
		self.assertIsNone(inventory.cfg_value('target_os = "linux"'))


if __name__ == "__main__":
	unittest.main()
