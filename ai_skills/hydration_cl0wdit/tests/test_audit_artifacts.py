"""Behavioral checks for report fidelity, incomplete coverage and campaign memory."""

import copy
import importlib.util
from pathlib import Path
import tempfile
import unittest

HELPER = Path(__file__).resolve().parents[1] / "scripts" / "audit_artifacts.py"
SPEC = importlib.util.spec_from_file_location("audit_artifacts", HELPER)
artifacts = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(artifacts)


class AuditArtifactsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.scan = self.root / "scan with spaces"
        self.scan.mkdir()
        self.ledger = self.root / "campaign" / "memory.json"
        self.scope = dict(
            scan_id="2611041010-example-scan", campaign="2611041000-example",
            timestamp="2026-11-04T10:10:00Z", repository="https://example.invalid/repository",
            local_checkout=str(self.root / "audited-checkout"), target_revision="a" * 40,
            comparison="git diff master...HEAD; merge base " + "b" * 40,
            mode="cl0wdit1.0", model="test-model", reasoning_effort="test-effort",
            skill_version="1.0", skill_revision="c" * 40, execution="sequential",
            passes_planned=1, roles_planned=["math", "access"], files=["src/path with spaces.rs"],
            memory=False, ledger_path=str(self.ledger), limitations="fixture scope", validation="fixture trace",
        )
        self.write_scope()

    def write_scope(self):
        artifacts.save_json(self.scan / "scope.json", self.scope)

    def item(self, bug_class="rounding-drift", kind="FINDING", evidence="exact proof", fix="precise fix"):
        item = dict(kind=kind, module="src/lib.rs::Pallet::Instance1", function="claim", bug_class=bug_class,
                    title="Claim uses the wrong scale", description="The caller receives excess rewards.",
                    locations=["src/lib.rs:10"], verification="source trace only",
                    candidate_ids=[f"pass-1/math/{bug_class}"], agent_roles=["math"])
        if kind == "FINDING":
            item.update(severity="High", confidence=95, proof=evidence, path="caller -> claim -> payout", fix=fix)
        else:
            item.update(next_check="Resolve the configured currency adapter", details={"boundary": "asset conversion"})
        return item

    def run_file(self, number=1, items=None, completed=None):
        completed = ["math", "access"] if completed is None else completed
        run = dict(number=number, target_revision=self.scope["target_revision"], model="test-model",
                   reasoning_effort="test-effort", roles_completed=completed,
                   roles_failed={role: "worker unavailable" for role in self.scope["roles_planned"] if role not in completed},
                   items=[] if items is None else items, rejected=[],
                   candidates=[identifier for item in (items or []) for identifier in item["candidate_ids"]])
        artifacts.save_json(self.scan / f"pass-{number}.json", run)

    def enable_memory(self):
        self.scope["memory"] = True
        self.write_scope()
        artifacts.snapshot_memory(self.scan)

    def test_report_should_keep_each_proof_and_fix_when_passes_rediscover_a_key(self):
        self.scope["passes_planned"] = 2
        self.write_scope()
        self.run_file(1, [self.item(evidence="first exact proof", fix="first safe fix")])
        self.run_file(2, [self.item(evidence="second exact proof", fix="second safe fix")], completed=["math"])
        report = artifacts.assemble(self.scan)
        for evidence in ("first exact proof", "second exact proof", "first safe fix", "second safe fix", "access: worker unavailable"):
            self.assertIn(evidence, report)
        self.assertEqual(report, (self.scan / "full-report.md").read_text())

    def test_report_should_retain_findings_and_leads_when_different_defects_share_a_function(self):
        self.run_file(items=[self.item(), self.item("wrong-origin"), self.item("oracle-boundary", "LEAD")])
        report = artifacts.assemble(self.scan)
        for label in ("rounding-drift", "wrong-origin", "oracle-boundary", "Resolve the configured currency adapter", "asset conversion"):
            self.assertIn(label, report)
        self.assertIn("src/path with spaces.rs", report)

    def test_plain_scan_should_ignore_ledger_when_memory_is_disabled(self):
        self.ledger.parent.mkdir()
        self.ledger.write_text("corrupt ledger that must not be read")
        self.run_file(items=[self.item()])
        artifacts.assemble(self.scan)
        self.assertEqual(self.ledger.read_text(), "corrupt ledger that must not be read")
        self.assertFalse((self.scan / "memory-before.json").exists())
        with self.assertRaisesRegex(ValueError, "memory is disabled"):
            artifacts.known(self.scan)

    def test_memory_should_count_once_per_scan_when_multiple_passes_and_retries_report_a_key(self):
        self.scope["passes_planned"] = 2
        self.enable_memory()
        self.run_file(1, [self.item()])
        self.run_file(2, [self.item()])
        artifacts.update_memory(self.scan)
        ledger = artifacts.update_memory(self.scan)
        self.assertEqual(ledger["records"][0]["scans"], 1)
        next_scan = self.root / "next-scan"
        next_scan.mkdir()
        self.scope["scan_id"] = "2611041110-example-scan"
        artifacts.save_json(next_scan / "scope.json", self.scope)
        artifacts.snapshot_memory(next_scan)
        artifacts.save_json(next_scan / "pass-1.json", artifacts.read_json(self.scan / "pass-1.json"))
        self.assertEqual(artifacts.update_memory(next_scan)["records"][0]["scans"], 2)

    def test_memory_should_keep_historical_records_when_current_scope_does_not_rediscover_them(self):
        self.enable_memory()
        self.run_file(items=[self.item()])
        previous = artifacts.update_memory(self.scan)
        next_scan = self.root / "partial-scan"
        next_scan.mkdir()
        self.scope["scan_id"] = "2611041210-partial-scan"
        self.scope["files"] = ["other.rs"]
        artifacts.save_json(next_scan / "scope.json", self.scope)
        artifacts.snapshot_memory(next_scan)
        run = artifacts.read_json(self.scan / "pass-1.json")
        run["items"] = []
        run["candidates"] = []
        artifacts.save_json(next_scan / "pass-1.json", run)
        self.assertIn("Earlier records not revalidated", artifacts.assemble(next_scan))
        self.assertEqual(artifacts.update_memory(next_scan)["records"], previous["records"])

    def test_snapshot_should_preserve_corrupt_ledger_when_validation_fails(self):
        self.scope["memory"] = True
        self.write_scope()
        self.ledger.parent.mkdir()
        self.ledger.write_text('{"schema": "unexpected", "records": []}')
        before = self.ledger.read_bytes()
        with self.assertRaisesRegex(ValueError, "invalid memory schema"):
            artifacts.snapshot_memory(self.scan)
        self.assertEqual(self.ledger.read_bytes(), before)
        self.assertFalse((self.scan / "memory-before.json").exists())

    def test_memory_should_reject_concurrent_change_when_another_scan_updated_ledger(self):
        self.enable_memory()
        self.run_file(items=[self.item()])
        ledger = artifacts.update_memory(self.scan)
        concurrent = copy.deepcopy(ledger)
        concurrent["last_scan_id"] = "another-scan"
        artifacts.save_json(self.ledger, concurrent)
        with self.assertRaisesRegex(ValueError, "ledger changed"):
            artifacts.update_memory(self.scan)
        self.assertEqual(artifacts.read_json(self.ledger), concurrent)

    def test_report_should_preserve_previous_output_when_a_pass_contains_duplicate_keys(self):
        self.run_file(items=[self.item()])
        original = artifacts.assemble(self.scan)
        self.run_file(items=[self.item(), self.item()])
        with self.assertRaisesRegex(ValueError, "duplicate key"):
            artifacts.assemble(self.scan)
        self.assertEqual((self.scan / "full-report.md").read_text(), original)

    def test_report_should_disclose_missing_pass_when_no_review_completed(self):
        report = artifacts.assemble(self.scan)
        self.assertIn("No review completed", report)
        self.assertIn("No saved pass; coverage incomplete", report)

    def test_memory_should_remain_absent_when_all_roles_failed(self):
        self.enable_memory()
        self.run_file(completed=[])
        with self.assertRaisesRegex(ValueError, "no completed review"):
            artifacts.update_memory(self.scan)
        self.assertFalse(self.ledger.exists())

    def test_validation_should_reject_silent_loss_when_raw_candidate_has_no_disposition(self):
        self.run_file(items=[self.item()])
        path = self.scan / "pass-1.json"
        run = artifacts.read_json(path)
        run["candidates"].append("pass-1/access/1")
        artifacts.save_json(path, run)
        with self.assertRaisesRegex(ValueError, "inventory and dispositions differ"):
            artifacts.assemble(self.scan)

    def test_assembly_should_sort_passes_numerically_when_ten_passes_are_saved(self):
        self.scope["passes_planned"] = 10
        self.write_scope()
        for number in (10, 2, 1):
            item = self.item(evidence=f"proof from pass {number}")
            self.run_file(number, [item])
        report = artifacts.assemble(self.scan)
        self.assertLess(report.index("proof from pass 1\n"), report.index("proof from pass 2\n"))
        self.assertLess(report.index("proof from pass 2\n"), report.index("proof from pass 10\n"))

    def test_validation_should_reject_findings_when_proof_is_missing(self):
        item = self.item()
        del item["proof"]
        self.run_file(items=[item])
        with self.assertRaisesRegex(ValueError, "proof"):
            artifacts.assemble(self.scan)


if __name__ == "__main__":
    unittest.main()
