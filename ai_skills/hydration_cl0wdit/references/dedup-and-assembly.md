# Adjudication, loop feedback, memory, and assembly

## Adjudicate each pass

Save raw worker/lens output in the external bundle directory. Inventory every candidate ID.
For each, record one disposition: confirmed, lead, duplicate-of, rejected-with-reason. Keep
this disposition map beside the pass file; verify no candidate disappeared. Pass JSON contains
surviving items with their source candidate IDs and rejected/duplicate dispositions.

Group by repository-relative module/pallet instance, exact function and defect class. Reuse a
known bug-class label only for the same underlying defect. Canonicalize synonyms before
writing the pass; do not normalize away Rust module paths or pallet instances. Different
mechanisms or fixes in one function stay separate, even when a coarse bug label is similar.
A shared helper defect can have one finding listing all affected callers; do not create
function-name duplicates unless the defects require distinct fixes.

Apply the four gates in `judging.md`. Look at every raw proof and suggested fix. Preserve
material mechanisms, alternate traces and safe fix alternatives in the adjudicated item.
Reject an unsafe fix explicitly; a raw worker's diff is not sacred. A composite chain needs a
proven connection and incremental impact; do not count the same loss twice. Neither agent
agreement nor repeat passes substitutes for a full trace. Uncertainty stays a lead.

Record passes in numeric order as `pass-1.json` through `pass-N.json`. See `report-formatting.md`
for the schema. Include role failures, actual model/effort, and the frozen revision. Record a
failed pass with all its roles failed rather than omitting it and implying clean coverage.
Validate before using its findings as feedback:

```sh
python3 SKILL_DIR/scripts/audit_artifacts.py validate --scan-dir SCAN_DIR
```

## Campaign memory

Memory is opt-in (`--memory` or multiple passes), unavailable to independent runs, and stored
outside the audited checkout in the campaign's external archive. It is not a source-code
cache and must not replace the current bundle. Do not use a global ledger that mixes unrelated
repositories/campaigns.

At scan start set `memory: true` and `ledger_path` in scope JSON. Run `snapshot-memory` to
validate the existing ledger and save the immutable `memory-before.json`; if no ledger exists,
this creates an empty snapshot. Invalid schemas, duplicate keys, invalid counts or a campaign
mismatch stop memory use before expensive review. Never delete/reset the ledger to proceed.
A plain one-pass scan with `memory: false` reads and writes no ledger or memory snapshot.

```sh
python3 SKILL_DIR/scripts/audit_artifacts.py snapshot-memory --scan-dir SCAN_DIR
python3 SKILL_DIR/scripts/audit_artifacts.py known --scan-dir SCAN_DIR
```

The `known` output combines the pre-scan ledger with saved current passes. Historical rows
say they are not revalidated; new-pass rows say they require a current proof. Append a factual
known-findings section to subsequent bundles only after all workers of the previous pass have
finished. Preserve source; the next pass has the same revision and a changed search history.

At the end of the scan, update memory from the immutable snapshot and all saved passes:

```sh
python3 SKILL_DIR/scripts/audit_artifacts.py memory-update --scan-dir SCAN_DIR
```

Each rediscovered key increments `scans` once, even if all N passes report it or finalization
runs twice. Record sightings and revisions; do not mark a missing key fixed, prune it by a
function-name regex, or revive a rejected candidate. Keep historical entries not revalidated
in a separate report section, outside confirmed findings. Conflicting concurrent ledger
updates must be reconciled, not overwritten. Failed/partial scans retain pass files for
recovery; an all-failed scan must not update memory.

## Final report

Before assembly, adjudicate the combined candidates across all passes. If later evidence
refutes an earlier candidate, save the previous pass artifact under `adjudication-history/`,
record the new evidence and disposition, and correct that scan's pass artifacts before
assembly. Preserve raw worker output. Do not leave a disproven candidate classified as a
finding merely because an earlier pass confirmed it. Never rewrite a previous completed
scan's report; a new scan records its own conclusions and references the old one.

Every scan saves scope, pass artifacts and an assembled report, regardless of memory or the
`--file-output` flag. The output flag copies this canonical report; it does not regenerate it.
This intentionally follows the workspace's durable-report policy instead of the old skill's
"no files without --file-output" rule.

```sh
python3 SKILL_DIR/scripts/audit_artifacts.py assemble --scan-dir SCAN_DIR
```

This writes `SCAN_DIR/full-report.md` atomically and prints it. The helper validates input
before replacing a report. It preserves every pass occurrence, including alternate proofs,
fixes and leads associated with a confirmed key. It sorts passes numerically and findings by
severity then confidence; missing or failed roles remain visible. Assembly does not promote,
reject or rewrite a candidate. Correct the adjudicated pass artifact with recorded evidence
if a conclusion changes; do not edit the final report independently.

Save the assembled report in the established campaign report destination, link the external
bundle and validation evidence, and update the campaign README and workspace task note.
Final consolidated campaign reports include confirmed findings only; keep leads and rejected
candidates in their original run artifacts. Keep contributing model/skill provenance and
prior finding references intact. Publication, DMs, fixes and on-chain transactions need their
own authorization; an audit invocation does not authorize them.
