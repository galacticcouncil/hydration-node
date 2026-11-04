# Known False Positives

Use these patterns to test a claim against the actual pinned SDK, runtime configuration and call path. A matching name or comment is not an automatic exemption. Record concrete refutations during adjudication.

---

## FP-001: Missing `#[transactional]` on dispatchables

**Pattern:** Flagging a `#[pallet::call]` dispatchable for lacking explicit `#[transactional]` annotation, claiming storage changes may not revert on error.

**Verification:** Inspect the pinned FRAME pallet-call expansion and the actual invocation path. Current FRAME dispatchable expansion can use `with_storage_layer`, which rolls back that call's storage on `Err`; direct helper calls and lifecycle hooks may have different boundaries. Missing an explicit attribute alone is not proof of partial writes. Conversely, a successful outer operation, ignored inner error, non-storage side effect, or panic can have different consequences. Confirm where state commits and whether block production can recover.

---

## FP-002: `saturating_*` arithmetic with documented intent

**Pattern:** Flagging `saturating_add`, `saturating_sub`, `saturating_mul` as masking errors when the saturating behavior is explicitly the intended design.

**Why it's wrong:** Saturating arithmetic is correct when clamping is the desired outcome — e.g., capping a counter at `u128::MAX`, decrementing a non-critical metric toward zero, or computing a fee floor. Only flag `saturating_*` when: (a) the operation is on a **balance or share calculation** where underflow/overflow should be an error, AND (b) there is no preceding guard (`ensure!`, `if` check, or explicit comment) validating the operands. A comment is a claim to verify, and a preceding bound must actually prove the operation safe. Check repeated rounding, accounting conservation and saturation on all reachable paths.

---

## FP-003: `.unwrap_or_default()` on storage reads

**Pattern:** Flagging `.unwrap_or_default()` on storage getters as a panicking operation.

**Why it's wrong:** `.unwrap_or_default()` never panics — it returns `Default::default()` when the value is `None`. This is idiomatic Substrate for reading storage values that may not exist. Only flag `.unwrap()` or `.expect()` without preceding validation.

---

## FP-004: Hardcoded/placeholder weights in benchmark code

**Pattern:** Flagging hardcoded weights (`Weight::from_parts(100_000, 0)`, `DbWeight::reads_writes(N, M)`) found inside `benchmarking.rs`, `weights.rs` (auto-generated), or test modules.

**Why it's wrong:** Benchmark and test modules intentionally use simplified weights. Only flag hardcoded/placeholder weights in **production pallet config** (`runtime/*/src/lib.rs`, pallet `Config` implementations) or in `#[pallet::weight(...)]` annotations on dispatchable functions in production code.

---

## FP-005: Unsigned extrinsics in test/mock modules

**Pattern:** Flagging `ensure_none(origin)?` or missing `ValidateUnsigned` in files within `tests/`, `mock/`, or `benchmarking/` directories.

**Why it's wrong:** Test and mock modules routinely use unsigned origins for simplicity. Unsigned extrinsic risk only applies to **production** dispatchable functions and their `ValidateUnsigned` implementations.

---

## FP-006: `SafeCallFilter = Everything` for local/internal XCM

**Pattern:** Flagging `type CallFilter = Everything` in XCM config as arbitrary execution risk without verifying the trust model.

**Why it's wrong:** Some runtimes intentionally use `Everything` for XCM messages originating from trusted internal sources (e.g., governance, local relay chain). Only flag this when the filter applies to messages from **untrusted or external origins** (sibling parachains, user-initiated XCM) AND there is a concrete path to call a dangerous extrinsic via XCM `Transact`.

---

## FP-007: Governance parameter changes as vulnerabilities

**Pattern:** Flagging that sudo/governance/admin can set pool fees, pause pallets, change economic parameters, or upgrade the runtime.

**Why it's wrong:** These are by-design governance capabilities, not vulnerabilities. Only flag privileged parameter changes when: (a) the change **retroactively harms locked/committed users** who cannot exit (e.g., raising commission to 100% while delegators are locked), OR (b) the change **corrupts existing state** (e.g., changing asset decimals while pools hold live balances), OR (c) there's a **missing sanity bound** (e.g., fee settable to >100%).

---

## FP-008: Division order inside fixed-point libraries

**Pattern:** Reporting division-before-multiplication solely from operator order, without using the actual library's scale and rounding rules.

**Verification:** Fixed-point wrappers preserve a finite scale, not exact real arithmetic. Calculate intermediate rounding and overflow with actual balances, decimals, and the pinned implementation. A wrapper is not an exemption. Report a concrete amplified loss, zero result or invariant violation; reject an allegation only when the actual computation/guard rules it out.

---

## FP-009: Storage deposits for bounded or ephemeral state

**Pattern:** Treating every storage write without a dedicated deposit as exploitable, or treating any bounded collection/ordinary fee as proof that growth is safe.

**Verification:** Determine the global number of keys/accounts, multiplicative per-key bounds, retention/cleanup, attacker budget and worst-case weight/proof. Per-account bounds do not cap account count; transaction fees and EDs may not price persistent storage. Reject only when the actual bounds or economics prevent material harm. There is no universal safe collection-size threshold.

---

## FP-010: Panicking operations with prior validation

**Pattern:** Flagging `.expect("reason; qed")` or `assert!` where a preceding check guarantees the condition holds.

**Why it's wrong:** Substrate convention uses `.expect("description; qed")` (quod erat demonstrandum) to document mathematically or logically proven invariants. If the code has a preceding `ensure!`, `if`/`match` guard, or the value was just validated in the same scope, the `expect` is a defensive assertion, not a vulnerability. Only flag `expect`/`unwrap` when: (a) the "proof" comment is missing or unconvincing, OR (b) the preceding guard doesn't actually cover the case, OR (c) it's on user-supplied input without prior validation.

---

## FP-011: Slippage on privileged/internal operations

**Pattern:** Reporting missing user slippage parameters on an internal operation without showing that its price or ordering can be manipulated.

**Verification:** Resolve the actual scheduler/hook ordering, pool state and available attacker transactions. Governance and protocol-internal trades can still be manipulated before execution. Require a concrete ordering, capital/cost and victim effect; do not dismiss them solely because the trigger has a trusted origin.

---

## FP-012: Stale oracle data on re-added assets (if reset logic exists)

**Pattern:** Flagging stale oracle state after asset removal and re-addition.

**Why it's wrong (when fixed):** If the runtime has been updated to clear oracle entries on `remove_token()` or reinitialize oracle state on `add_token()`, this is a known-fixed issue. Check for oracle accumulator cleanup or equivalent in the remove/add flow before reporting.

---

## FP-013: ExistentialDeposit "too low" without economic analysis

**Pattern:** Flagging a specific ED value as insufficient without calculating the actual cost of an attack.

**Why it's wrong:** ED adequacy depends on token price, storage costs, and the specific chain's economics. An ED of 1_000_000 (1e6) may be trivial for a low-value token or meaningful for an expensive one. Only flag ED as insufficient when you can demonstrate: (a) the cost to create N accounts at the current ED, AND (b) N is enough to meaningfully degrade chain performance, AND (c) the cost is economically trivial relative to the attack impact.
