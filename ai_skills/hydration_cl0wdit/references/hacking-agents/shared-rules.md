# Shared review rules

Read the frozen source, manifest and specialty instructions in your bundle. Inspect relevant
callers, runtime trait implementations, feature flags, configuration, dependency versions and
base-side context when needed; record additions to reviewed context. Request missing context
instead of inventing it. Do not claim a full read when the bundle was truncated.

The audited checkout is read-only. Put tests, PoCs and scratch files in the external archive
or an isolated copy, never in the original checkout even temporarily. Use local tools as
mapped by the host: Read -> bounded file reads; Glob/Grep -> rg; Bash -> command execution.
Do not spawn further workers unless explicitly authorized. Return results to the orchestrator;
it owns the durable artifacts and report.

Check dispatchables, lifecycle hooks, migrations, unsigned validation, runtime APIs, scheduled
calls, XCM/EVM/precompile entry points, internal helpers and cross-pallet calls. Resolve the
actual concrete runtime implementation before importing a Solidity or generic FRAME assumption.

For a discovered pattern, search sibling paths and other in-scope implementations. Keep
separate root causes/fixes separate. A suggested fix must be checked for new loss of funds,
blocked execution, bad weight accounting and missing recovery. Ordinary authorized upgrades
and hooks can expose reportable operational defects; "privileged" does not dismiss them.

## Output

Return structured candidate blocks with no generic preamble. Vector roles may begin with their
classification table. Assign each candidate an ID such as `pass-1/math/1`; retain it through
adjudication. Use a repository-relative module path (including pallet instance when relevant),
exact function name, and kebab-case bug class. Reuse known labels for the same root cause;
never merge different defects merely because they share a function or a broad class.

```text
FINDING | id: pass-1/access/1 | module: pallets/foo/src/lib.rs::Foo | function: withdraw | bug_class: missing-owner-check
locations: pallets/foo/src/lib.rs:123
severity: High
confidence: 95
path: actor -> entry point -> relevant guards -> state change -> victim impact
proof: concrete amounts/state sequence and citations from the reviewed revision
assumptions: required state and which deployment assumptions were verified
description: actor, trigger and effect in plain language
fix: minimal safe diff or precise implementation steps
verification: executed reproduction (command/result), or source trace only

LEAD | id: pass-1/math/1 | module: pallets/foo/src/lib.rs::Foo | function: claim | bug_class: rounding-drift
locations: pallets/foo/src/lib.rs:234
description: concrete mechanism and what is unproven
next_check: exact missing evidence or reproduction to resolve it
fix: suggested mitigation, only when supportable
verification: which parts were inspected and which were not
```

Use the same fields for operational defects, identifying the lifecycle trigger instead of
inventing an attacker. No complete path/proof means a LEAD; no agent vote count overrides it.
Include the additional fields your specialty requests. Keep code and numeric evidence exact.

A known-finding section is historical context, not a suppress-list or proof of validity. Hunt
new paths, but report known bugs you can still demonstrate. Explicit independent reviews
receive no historical section. A later pass that receives feedback is not independent.
