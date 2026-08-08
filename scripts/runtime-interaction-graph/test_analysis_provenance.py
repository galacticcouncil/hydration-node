import importlib.util
import tempfile
import unittest
from pathlib import Path


MODULE = Path(__file__).with_name("analysis_provenance.py")
SPEC = importlib.util.spec_from_file_location("analysis_provenance", MODULE)
provenance = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(provenance)


class AnalysisProvenanceTests(unittest.TestCase):
	def test_tool_fingerprint_should_cover_every_local_input(self):
		with tempfile.TemporaryDirectory() as directory:
			root = Path(directory)
			collector = root / "collector.py"
			helper = root / "helper.py"
			collector.write_text("import helper\n")
			helper.write_text("VALUE = 1\n")
			before = provenance.tool_input_fingerprint([collector, helper])
			helper.write_text("VALUE = 2\n")
			after = provenance.tool_input_fingerprint([collector, helper])
			self.assertNotEqual(before["sha256"], after["sha256"])
			self.assertEqual(set(after["files"]), {"collector.py", "helper.py"})
			self.assertTrue(provenance.valid_tool_input_fingerprint(after))
			tampered = {**after, "files": {**after["files"], "helper.py": "0" * 64}}
			self.assertFalse(provenance.valid_tool_input_fingerprint(tampered))

	def test_tool_fingerprint_should_reject_invalid_names_and_hashes(self):
		with self.assertRaisesRegex(ValueError, "invalid tool input name"):
			provenance.tool_input_digest({"../collector.py": "0" * 64})
		with self.assertRaisesRegex(ValueError, "invalid tool input checksum"):
			provenance.tool_input_digest({"collector.py": "not-a-hash"})

	def test_tree_fingerprint_should_change_when_source_changes(self):
		with tempfile.TemporaryDirectory() as directory:
			root = Path(directory)
			(root / "pallets/example/src").mkdir(parents=True)
			source = root / "pallets/example/src/lib.rs"
			source.write_text("fn first() {}\n")
			before = provenance.tree_fingerprint(root)
			source.write_text("fn second() {}\n")
			after = provenance.tree_fingerprint(root)
			self.assertNotEqual(before["sha256"], after["sha256"])
			self.assertEqual(after["file_count"], 1)

	def test_tree_fingerprint_should_cover_integration_test_sources(self):
		with tempfile.TemporaryDirectory() as directory:
			root = Path(directory)
			(root / "integration-tests/src").mkdir(parents=True)
			source = root / "integration-tests/src/lib.rs"
			source.write_text("fn first_test() {}\n")
			before = provenance.tree_fingerprint(root)
			source.write_text("fn changed_test() {}\n")
			self.assertNotEqual(before["sha256"], provenance.tree_fingerprint(root)["sha256"])

	def test_reusable_artifact_should_require_matching_content_and_inputs(self):
		with tempfile.TemporaryDirectory() as directory:
			artifact = Path(directory) / "package.mir"
			artifact.write_text("mir")
			entry = {
				"status": "ok",
				"input_fingerprint": "inputs",
				"artifact_sha256": provenance.file_sha256(artifact),
			}
			self.assertTrue(provenance.reusable_artifact(entry, "inputs", artifact))
			artifact.write_text("stale")
			self.assertFalse(provenance.reusable_artifact(entry, "inputs", artifact))
			self.assertFalse(provenance.reusable_artifact(entry, "changed-inputs", artifact))


if __name__ == "__main__":
	unittest.main()
