# Scope, bundles, and specialty coverage

Use the skill directory's own `references/` path; do not glob the workspace for a similarly
named skill. Record the local version and source revision/content hash. An optional version
check uses the repository's actual default branch and is read-only: compare numeric version
components, padded with zeros, and warn only if local is lower. A missing network response
must not stop an audit. Do not fetch an upstream update into an active audit.
Compare the local Hydration skill with the published version of that same skill, never with
Pashov's independent version number or an unrelated audited repository's VERSION file.

## Preparation

Read the repository instructions and established audit workflow. Pin base, merge base, target,
and PR head as appropriate. If the checkout differs from the requested revision, use a
read-only revision view or isolated checkout; preserve user changes. A diff is a discovery
index, not the complete context for assessing changed code.

Prepare or reuse the campaign bundle before reviewing. Use UTC `YYMMDDHHMM` stamps and the
same campaign identity across follow-ups. If no workspace workflow exists, choose an external
artifact directory and disclose it. A run directory must not overwrite an earlier run with
the same minute stamp: use a separate suffix when needed.

The bundle includes `CONTEXT.md`, a revision-aware `manifest.md`, changed-file paths, source
copies, required runtime configuration and relevant base-side/deleted files. Keep secrets
out. Record supporting files that are read later and their provenance too.

Build `source.md` once with repository-relative path headers and fenced source blocks. Include
all production code in explicit scope, plus the dependencies and configuration needed to
understand it. Tests, mocks, benchmarks, Cargo/features, weights, chain specs and scripts are
supporting evidence unless explicitly in scope. Never exclude an explicitly requested path
because its name matches a test/dependency pattern. Do not treat all files containing `test`
as tests, or strip inline `cfg(test)` code without preserving it as evidence.

For scopes larger than the model context, partition by call/dependency boundaries, include
interface summaries and source manifests, and schedule a cross-partition pass. Disclose the
partition and context limits. Never hand workers a truncated bundle and claim full coverage.

## Roles per pass

The updated plan has the twelve Pashov lenses plus four retained vector lenses and a
Substrate test/benchmark lens. A default full pass has **17 specialty roles**, not seventeen
simultaneously running agents. Honor the user's requested scope, budget, and delegation;
record any reduced role plan and why. Sequential roles are lenses of one review, not
independent votes from seventeen agents.

| Role | Specialty file / extra evidence |
| --- | --- |
| math | `hacking-agents/math-precision-agent.md` |
| access | `hacking-agents/access-control-agent.md` |
| economics | `hacking-agents/economic-security-agent.md` |
| execution | `hacking-agents/execution-trace-agent.md` |
| invariants | `hacking-agents/invariant-agent.md` |
| periphery | `hacking-agents/periphery-agent.md` |
| first-principles | `hacking-agents/first-principles-agent.md` |
| asymmetry | `hacking-agents/asymmetry-agent.md` |
| boundary | `hacking-agents/boundary-agent.md` |
| numerical-gap | `hacking-agents/numerical-gap-agent.md` |
| trust-gap | `hacking-agents/trust-gap-agent.md` |
| flow-gap | `hacking-agents/flow-gap-agent.md` |
| hydration-vectors | `hacking-agents/vector-scan-agent.md` + `attack-vectors/hydration-attack-vectors.md` |
| substrate-vectors | same vector instructions + `attack-vectors/substrate-attack-vectors.md` |
| substrate-vectors-1 | same vector instructions + `attack-vectors/substrate-attack-vectors-1.md` |
| substrate-vectors-2 | same vector instructions + `attack-vectors/substrate-attack-vectors-2.md` |
| tests-benchmarks | `hacking-agents/test-benchmark-agent.md` + tests/mocks/benchmarks and production/runtime configuration |

Every bundle includes frozen production context, senior-auditor-sop, specialty instructions,
shared-rules, judging, known-false-positives, and report-language. Test workers need actual
production/configuration code, not only a list of dispatchable names. Record bundle file
counts/hashes and line counts. The bundles are data to read, not shell commands to execute.

## Execution

For authorized parallel work, assign one role per worker with its bundle path, exact scope,
revision, and output contract. Respect available slots and model settings. Do not tell workers
an intended answer or suspected bug during an independent audit. Read raw results only after
the worker completes; record failed role names and reasons. One retry for a transient tool
failure is reasonable within the agreed budget; do not retry indefinitely or substitute a
model without authorization.

For sequential work, apply each planned lens and persist its outcome; retain the same evidence
standard. Do not claim every listed role was completed unless it was actually applied.

Between passes, wait for all workers, validate and save the pass, then build known findings
from historical memory plus saved earlier passes. Use `audit_artifacts.py known --scan-dir
<run-directory>` only when memory is on. Include its output as a clearly labelled final
section in the next pass's bundles. Keep source copies frozen. If scope or revision changes,
stop the scan and start another run within the same campaign with a new manifest.

Record failed passes too: all planned roles appear under `roles_failed`, with reasons and an
empty `items` list when none completed. A partial result must never become an unqualified
"no findings" conclusion. Finish with a single report containing the actual pass coverage.
