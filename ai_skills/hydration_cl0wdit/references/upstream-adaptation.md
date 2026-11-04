# Pashov v4.1 adaptation

Upstream inspected: `/home/cl0w/projects/skills/pashov/solidity-auditor`, version **4.1**,
repository revision `92b0ea6786e6c210bc9a839e4813d50876c1c550`.
Source: https://github.com/pashov/skills/tree/92b0ea6786e6c210bc9a839e4813d50876c1c550/solidity-auditor
The MIT license notice is retained at `../LICENSE.pashov`.

This update keeps the `hydration_cl0wdit` entrypoint and its Codex/Claude symlinks. It raises
the local version from 0.2.0 to 1.0; it does not imply version equivalence with Pashov.

Adopted concepts:

- Senior-auditor reasoning, plain-language findings and read-only review workers.
- Periphery, asymmetry, boundary and three combined-lens (numerical/trust/flow) specialties.
- Multiple passes, known-findings feedback, opt-in historical memory and canonical labels.
- Durable per-pass findings, deterministic final assembly, explicit failed-role coverage and
  separation of scan counts from repeated-pass sightings.
- Explicit files bypass discovery exclusions; runtime/deployment integration code remains in
  scope when it affects the change. Numeric version checks warn only when local is lower.

Intentional adaptations:

- Preserve all four Substrate/Hydration attack-vector collections and the test/benchmark lens:
  17 roles in a full pass, scheduled within actual slots rather than blindly spawning 17.
- Require explicit user authorization for delegation, inherit the actual model, and default
  to one pass. No forced model picker, unsolicited multi-pass cost, or borrowed Solidity
  duration estimates. Respect independent-run requirements and established campaign scope.
- Use JSON artifacts and a standard-library Python assembler instead of transplanting
  Solidity-specific shell/TSV parsing and contract/function-name extraction into Rust.
  Keep identifiers case-sensitive and module/instance-qualified. Preserve per-pass evidence.
- Store source/PoCs/memory in external archives and reports in campaign destinations, matching
  workspace policy. `--file-output` only adds a copy. The audited checkout stays read-only.
- Keep unrediscovered memory as historical, not automatically pruned/fixed. Source names,
  partial scans and renamed Rust impls cannot establish whether a prior bug was repaired.
- Require proof for confirmation. Do not promote by agent votes, treat uncertainty as success,
  dismiss ordinary upgrade failures as trusted-role behavior, or preserve an unsafe fix just
  because an agent wrote it. Every confirmed finding gets a suggested fix; leads get a next
  check and a supported mitigation. Missing tests alone are not vulnerabilities.
- Check actual FRAME transaction boundaries, currency adapters, fixed-point rounding and
  global storage growth rather than applying Solidity token or blanket safe-pattern claims.

The helper's artifact validation is a structural check. Its tests exercise persistence,
assembly, optional-memory boundaries and failure disclosure; they are not a new security audit
of runtime code and do not demonstrate the effectiveness of every review lens.

Compatibility note: the existing `hydration_cl0wdit` frontmatter name contains underscores.
The generic skill-creator validator rejects that legacy spelling. Preserve it for existing
invocations; validate the other frontmatter requirements in a temporary copy with only that
name normalized. Do not claim that the unchanged naming exception passed the strict validator.
