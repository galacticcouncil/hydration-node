# Audit artifacts and report contract

Every scan has one external run directory containing `scope.json`, `pass-K.json` files, raw
candidate results/dispositions, and `full-report.md`. Follow the workspace campaign layout
when present. Canonical findings reports are persisted even without `--file-output`; that
flag creates an additional copy at the requested path. Use UTC `YYMMDDHHMM` for artifact names.

The deterministic helper is `scripts/audit_artifacts.py`, relative to the skill root. It uses
only Python's standard library. It does not judge findings, invoke models, modify source code,
or contact a chain. It validates JSON and assembles the report from saved pass artifacts.

## Scope JSON

Write factual values, not the example placeholders. Files are an array so paths with spaces
are preserved. Record revisions and reviewed-file provenance in the linked bundle manifest.
The `model` and `reasoning_effort` fields come from the runtime/user/workspace instructions;
do not guess model suffixes. Additional factual scope metadata is allowed.

```json
{
  "scan_id": "2611041010-feature-scan-1",
  "campaign": "2611041000-feature",
  "timestamp": "2026-11-04T10:10:00Z",
  "repository": "https://github.com/owner/repository",
  "local_checkout": "/absolute/path/to/audited/repository",
  "target_revision": "full-target-hash",
  "comparison": "git diff full-base-hash...full-target-hash (merge base full-merge-base-hash)",
  "mode": "cl0wdit1.0",
  "model": "exact-model-identifier",
  "reasoning_effort": "actual-effort",
  "skill_version": "1.0",
  "skill_revision": "skill-commit-and-content-hash-if-locally-modified",
  "execution": "sequential",
  "passes_planned": 1,
  "roles_planned": ["math", "access"],
  "files": ["pallets/example/src/lib.rs"],
  "memory": false,
  "limitations": "Scope, contexts not read, unresolved assumptions and unavailable roles.",
  "validation": "Exact commands and results, or source-trace-only limitations."
}
```

The example role list is abbreviated: the normal full plan has 17 roles listed in
`orchestration.md`. A reduced plan needs a scope/budget explanation. When memory is enabled,
add `ledger_path` pointing to the external campaign ledger; use `snapshot-memory` before the
first pass. Keep the scan directory outside `local_checkout`.

## Pass JSON

Each saved pass names all completed/failed roles, exact revision/model/effort, adjudicated
items, and rejected/duplicate candidate dispositions. No empty missing-role lists that imply
an unexecuted lens was completed. Use a failed role reason if the role did not run.

```json
{
  "number": 1,
  "target_revision": "full-target-hash",
  "model": "exact-model-identifier",
  "reasoning_effort": "actual-effort",
  "roles_completed": ["math"],
  "roles_failed": {"access": "required worker unavailable"},
  "candidates": ["pass-1/math/1", "pass-1/math/2"],
  "items": [{
    "kind": "FINDING",
    "module": "pallets/example/src/lib.rs::Example",
    "function": "claim",
    "bug_class": "incorrect-reward-scale",
    "title": "Claim uses the wrong reward scale",
    "description": "The claimant receives excess rewards because claim applies the asset scale twice.",
    "locations": ["pallets/example/src/lib.rs:100"],
    "severity": "High",
    "confidence": 95,
    "path": "concrete caller -> guards -> writes/transfers -> victim impact",
    "proof": "Concrete values, code citations and before/after state.",
    "assumptions": "Required state and its evidence.",
    "fix": "Precise implementation steps or a complete safe diff.",
    "verification": "Actual reproduction command/result, or source trace only.",
    "candidate_ids": ["pass-1/math/1"],
    "agent_roles": ["math"]
  }],
  "rejected": [{"candidate_id": "pass-1/math/2", "reason": "duplicate of pass-1/math/1"}]
}
```

A `LEAD` uses the same identity/location/provenance fields plus `next_check`. It has no
`severity` or `confidence`. Include `proof`, `path`, `fix` or `assumptions` only where supported.
Use `details` for specialty-specific evidence fields, and `seam` for combined lenses. Every
confirmed finding requires path, proof, verification and suggested fix. The helper's
`CONFIDENCE_THRESHOLD` is the single executable threshold. It validates shape, not truth.

## Assembled report

The helper puts model, effort and skill provenance directly below the title, followed by scope,
actual pass/role coverage, validation/limitations, a findings overview, and detailed findings.
Each finding retains all saved pass occurrences so alternate proof/fix text cannot vanish
under context pressure. Leads and unvalidated historical records appear separately. A missing
or all-failed review is displayed as incomplete; it must never be described as a clean audit.

Do not conflate confidence, severity, agent agreement, number of passes, ledger scan counts,
or returned DB weights with measured runtime performance. Source traces are valid evidence
when complete; label them accurately rather than claiming tests were executed.

The `candidates` inventory lists every raw candidate ID. Every ID must occur exactly once
in an item's `candidate_ids` or in `rejected`; validation rejects silent drops and duplicate
dispositions. Do not omit raw candidates merely because adjudication rejected them.
