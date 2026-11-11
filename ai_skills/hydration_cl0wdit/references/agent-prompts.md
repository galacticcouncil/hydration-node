# Worker prompts

Use these only for an audit run with authorized delegation. Otherwise apply the same instructions
as sequential review lenses. Substitute actual scope, source revision, role, model and bundle
path; never claim independent agents for sequential work.

```text
Review the frozen Substrate/Rust audit bundle at {bundle_path} ({line_count} lines).
Role: {role}. Pass: {K}/{N}. Target revision: {full_sha}.
The manifest distinguishes changed scope from supporting context. Read the complete bundle,
including the senior-auditor SOP, your specialty, shared rules and judging criteria. Read
additional concrete implementations and callers when needed; identify their revisions.

The audited checkout is read-only. Place PoCs, tests and scratch artifacts only in
{external_scratch_path}, or return them as text. Do not spawn additional workers.

Return candidate blocks using shared-rules.md, including candidate IDs, locations, concrete
proof, severity/confidence for confirmed findings, and a safe suggested fix. An incomplete
path is a lead with the next check required. Trace normal hooks and upgrades for operational
failures as well as unprivileged attacks. Do not infer that comments, trusted origins,
fixed-point wrappers, bounded collections or transaction rollback prove safety.

Report what you actually reviewed and which tests you actually ran. If the bundle is missing
or truncated, report the limitation instead of claiming a clean result.
```

For numerical/trust/flow gap roles, add:

```text
Name the combined lenses and show the evidence at their intersection. Keep any supported
single-lens defect you encounter for deduplication; do not suppress a real bug because another
role normally covers it.
```

Only when memory is enabled and actual prior records exist, add:

```text
The final section contains historical findings or results from earlier passes. These are
context, not current proof or a suppression list. Prioritize new paths but report a listed
bug you can still demonstrate. Reuse its bug-class label for the same root cause; distinguish
current evidence from earlier assertions. This pass is not independent of the supplied history.
```

Do not append a known-findings paragraph to independent runs or to bundles with no such section.
