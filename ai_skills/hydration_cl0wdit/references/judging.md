# Finding Validation

Every finding passes four sequential gates. Fail any gate → **rejected** or **demoted** to lead. Later gates are not evaluated for failed findings.

## Gate 1 — Refutation

Construct the strongest argument that the finding is wrong. Find the guard, check, or constraint that kills the attack — quote the exact line and trace how it blocks the claimed step.

- Concrete refutation (specific guard blocks exact claimed step) → **REJECTED** (or **DEMOTE** if code smell remains)
- Speculative refutation ("probably wouldn't happen") → **clears**, continue

## Gate 2 — Reachability

Prove the vulnerable state exists in a live deployment.

- Structurally impossible (enforced invariant prevents it) → **REJECTED**
- Requires privileged actions outside normal operation → **DEMOTE**
- Achievable through normal usage or common token/XCM behaviors → **clears**, continue

## Gate 3 — Trigger

Prove an unprivileged actor executes the attack.

- Only trusted roles can trigger (sudo, governance, council, admin origin) → **DEMOTE**
- Costs exceed extraction → **REJECTED**
- Unprivileged actor triggers profitably → **clears**, continue

## Gate 4 — Impact

Prove material harm to an identifiable victim.

- Self-harm only → **REJECTED**
- Dust-level, no compounding → **DEMOTE**
- Material loss to identifiable victim → **CONFIRMED**

## Confidence

Start at **100**, deduct: partial attack path **-20**, bounded non-compounding impact **-15**, requires specific (but achievable) state **-10**. Confidence ≥ 80 gets description + fix. Below 80 gets description only.

## Safe patterns (do not flag)

- `saturating_*` with explicit justification comment or preceding bounds check
- `.unwrap_or_default()` on storage reads (never panics)
- `.expect("description; qed")` with a sound preceding proof
- `#[pallet::call]` dispatchables without `#[transactional]` (implicit since FRAME v2)
- Hardcoded weights in `benchmarking.rs`, `weights.rs`, or test modules
- `ensure_none` in test/mock modules
- `SafeCallFilter = Everything` for trusted internal XCM origins
- Governance parameter changes that are by-design privileges
- Division-before-multiply inside `FixedU128` or similar fixed-point wrappers
- Bounded storage (`BoundedVec`, `BoundedBTreeMap`) with tight limits and gating deposit

## Lead promotion

Before finalizing leads, promote where warranted:

- **Cross-pallet echo.** Same root cause confirmed as FINDING in one pallet → promote in every pallet where the identical pattern appears.
- **Multi-agent convergence.** 2+ agents flagged same area, lead was demoted (not rejected) → promote to FINDING at confidence 75.
- **Partial-path completion.** Only weakness is incomplete trace but path is reachable and unguarded → promote to FINDING at confidence 75, description only.

## Leads

High-signal trails for manual investigation. No confidence score, no fix — title, code smells, and what remains unverified.

## Known False Positives

Before scoring, check the finding against `known-false-positives.md` (included in your bundle). If it matches a listed pattern, drop it — do not score or report it.

## Do Not Report

Anything a linter, compiler, or `cargo clippy` would dismiss. Admin/governance privileges by design. Missing event emissions. Centralization without exploit path. Implausible preconditions (but fee-on-transfer via XCM, rebasing assets, freezing/thawing ARE plausible for runtimes handling arbitrary assets or XCM messages).
