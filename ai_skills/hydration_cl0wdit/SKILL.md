---
name: hydration_cl0wdit
description: Audit Substrate runtimes, Rust pallets, migrations, runtime adapters, and their operational boundaries. Supports explicit file or PR scope, repeated passes with --loop, and opt-in campaign memory. Use for security audits and targeted security reaudits.
allowed-tools: Read, Glob, Grep, WebFetch, Bash, Agent
---

# Substrate security audit

Audit the specified revision and report reproducible security or operational defects. This
workflow adapts Pashov Solidity Auditor v4.1 to FRAME, XCM, ORML, and Hydration. Its provenance
and intentional differences are in [upstream-adaptation.md](references/upstream-adaptation.md).

These audit instructions apply when running an audit, not when inspecting or editing this skill.
Read the local `VERSION` at execution time and use the report label `cl0wdit{version}`.

## Scope and authorization

Follow repository instructions and the workspace campaign workflow when available. On this
workspace that workflow is `/home/cl0w/workspace-docs/workflows/audits.md`. Reuse the campaign
and bundle when scope and revisions match; keep copied source and reproduction artifacts in
its external archive. Keep reports in the established campaign destination.

- No arguments: follow the workspace comparison default (`master...HEAD`), recording the base,
  merge base, target hashes, and exact command. Outside that workspace, default to the current
  repository's production Rust sources. State which interpretation was used.
- Named files/directories: audit exactly those paths, even if they are tests or dependencies.
  Read relevant callers, implementation dependencies, runtime configuration, and base-side
  code as supporting context; list context separately from changed scope.
- `--pr <number|URL>`: resolve the repository and base/head hashes using connected GitHub tools
  or `gh`, with the GitHub API as a fallback. Handle pagination, renames, and deleted files.
  Pin source to the PR revision; do not silently review an unrelated local checkout.
- Use `rg --files` for discovery. Exclude `.git`, `target`, `node_modules`, generated build
  outputs, and vendored dependencies from default discovery, not from explicit requests or
  necessary context. Classify test/mock/benchmark files as evidence; do not discard them.
  Inline `#[cfg(test)]` modules require inspection too. Runtime migration and deployment
  scripts, chain specs, adapters, precompiles, build configuration, and public runtime APIs
  remain eligible when they affect the requested change.
- Audit workers are read-only in the audited checkout. Put PoCs and scratch work in the
  external archive or an isolated copy, including when a PoC would later be deleted. Check
  for existing user changes and record which revision supplied every reviewed file.
- Delegation requires the user's explicit request under this repository's AGENTS.md. A skill
  invocation alone does not authorize spawning agents. Otherwise apply the same lenses
  sequentially and report that execution mode. Respect actual concurrency limits.
- Inherit the current model unless the user specifies another available model. Record actual
  model identifiers and reasoning efforts; never silently substitute or invent provenance.

## Passes and memory

A **scan** is one invocation against a fixed source snapshot. A **pass** is one review with
all planned specialty lenses. Multiple passes that share earlier results are not independent.

- Default: one pass, no historical ledger access. No model or pass-count question is needed.
- `--loop` means three passes; `--loop N` or an explicit natural-language count means N,
  from 1 through 10. For a vague request for several passes, clarify the count before the
  repeated work. Do not copy upstream Solidity duration/cost estimates to Substrate audits.
- `--memory` enables campaign-scoped finding memory. More than one pass also enables memory.
  Loop feedback includes results from earlier passes of this scan. Earlier scans are clearly
  labelled historical, never accepted as proof at the current revision.
- An explicitly independent review reads no historical findings or ledger. Honor that mode
  before applying loop defaults; ask only if the user's requested modes conflict.
- Preserve records that are not rediscovered, including during partial-file scans. Absence,
  a rename, or a function-name regex is not evidence of a fix. A targeted reaudit records a
  supported disposition for each requested prior finding in its report.
- `--file-output [path]` makes an additional copy of the assembled report. It does not control
  canonical campaign/run persistence: every scan records its scope, passes, and full report.
  Do not place additional files in the audited checkout without this flag or user direction.

## Workflow

1. **Freeze scope and prepare the bundle.** Record revisions, paths, changed vs supporting
   files, source status, skill version/revision, model/effort, execution mode, roles, and pass
   count. Read [orchestration.md](references/orchestration.md) for bundles and the role matrix.
   Preserve the four Substrate/Hydration vector collections as audit checklists.
2. **Load the review rules.** Read [judging.md](references/judging.md),
   [report-formatting.md](references/report-formatting.md), and
   [report-language.md](references/report-language.md). Each worker receives the
   [senior auditor SOP](references/senior-auditor-sop.md), its specialty, the
   [shared rules](references/hacking-agents/shared-rules.md), and relevant factual source.
3. **Review each pass.** Use [agent-prompts.md](references/agent-prompts.md). With authorized
   delegation, schedule waves within available slots and wait for all launched work before
   updating bundles. Otherwise use sequential lenses without claiming independent agents.
   Save raw results and disclose failed or unexecuted roles. Revisit a concrete unresolved
   question when evidence changes; do not declare `UNCERTAIN` equivalent to `ALLOWS`.
4. **Adjudicate and persist.** Follow [dedup-and-assembly.md](references/dedup-and-assembly.md).
   Canonicalize labels without merging distinct root causes or losing proofs/fixes. Account
   for each candidate as confirmed, lead, duplicate, or rejected with evidence. Agent counts
   and repeat observations do not establish validity. Save a structured pass file.
5. **Assemble one report.** The standard-library helper
   `scripts/audit_artifacts.py` validates the artifacts and assembles every saved finding and
   lead; do not regenerate the final report from memory. Report incomplete coverage plainly.
   Update optional campaign memory from the immutable pre-scan snapshot, once per scan.
   Persist the report and workspace task note even when no findings are confirmed.

A later pass gets known bug-class labels and evidence, then checks new functions, paths,
boundaries, and combinations. It must still report a known bug it can reproduce. Stop after
N passes, a user stop request, an unavailable required input, or a failed pass that prevents
meaningful progress. Preserve partial results and actual coverage; never label failure as a
clean audit. Do not modify production code or publish/send reports unless requested.
