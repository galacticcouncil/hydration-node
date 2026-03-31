# Finding Validation

Each finding passes a false-positive gate, then gets a confidence score (how certain you are it is real).

## FP Gate

Every finding must pass all three checks. If any check fails, drop the finding — do not score or report it.

1. You can trace a concrete attack path: caller → extrinsic/hook → internal call chain → storage mutation → loss/impact. Evaluate what the code _allows_, not what the deployer _might configure_.
2. The entry point is reachable by the attacker (check origin requirements: `ensure_signed`, `ensure_root`, custom origin filters, `T::AdminOrigin`, governance-gated calls).
3. No existing guard already prevents the attack (`ensure!`, `frame_support::transactional`, `checked_*`/`saturating_*` arithmetic, `BoundedVec` limits, weight metering, etc.).

## Confidence Score

Confidence measures certainty that the finding is real and exploitable — not how severe it is. Every finding that passes the FP gate starts at **100**.

**Deductions (apply all that fit):**

- Privileged caller required (sudo, governance, admin origin, council) → **-25**.
- Attack path is partial (general idea is sound but cannot write exact caller → extrinsic → storage mutation → outcome) → **-20**.
- Impact is self-contained (only affects the attacker's own funds/state, no spillover to other users) → **-15**.

Confidence indicator: `[score]` (e.g., `[95]`, `[75]`, `[60]`).

Findings below the confidence threshold (default 75) are still included in the report table but do not get a **Fix** section — description only.

## Known False Positives

Before scoring, check the finding against `known-false-positives.md` (included in your bundle). If it matches a listed pattern, drop it — do not score or report it.

## Do Not Report

- Anything a linter, compiler (`cargo clippy`), or seasoned Rust developer would dismiss — INFO-level notes, minor style issues, naming conventions, missing doc comments.
- Sudo/root/governance can set parameters, pause pallets, or upgrade runtime — these are by-design privileges, not vulnerabilities.
- Missing event emissions or insufficient logging.
- Centralization observations without a concrete exploit path (e.g., "sudo could drain treasury" with no specific mechanism beyond inherent trust assumptions).
- Theoretical issues requiring implausible preconditions (e.g., compromised collator/validator set, >50% token supply held by attacker, corrupted WASM blob). Note: common token behaviors (fee-on-transfer via XCM, rebasing assets, freezing/thawing) and cross-chain message manipulation are NOT implausible — if the runtime handles arbitrary assets or XCM messages, these are valid attack surfaces.
