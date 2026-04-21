# Known False Positives

Before reporting a finding, check it against this list. If a finding matches a pattern below, it is a known false positive — drop it silently.

---

## FP-001: Missing `#[transactional]` on dispatchables

**Pattern:** Flagging a `#[pallet::call]` dispatchable for lacking explicit `#[transactional]` annotation, claiming storage changes may not revert on error.

**Why it's wrong:** Since FRAME v2 (~Substrate 0.9.25+), all `#[pallet::call]` dispatchables are automatically wrapped in a transactional layer. Storage mutations revert on `Err` by default. The `#[transactional]` attribute was subsequently deprecated because the behavior is implicit. Only flag missing transactional semantics for **non-dispatchable** internal functions called outside of a dispatchable context (e.g., hooks like `on_initialize`, `on_finalize`, offchain workers) where the automatic wrapper does not apply.

---

## FP-002: `saturating_*` arithmetic with documented intent

**Pattern:** Flagging `saturating_add`, `saturating_sub`, `saturating_mul` as masking errors when the saturating behavior is explicitly the intended design.

**Why it's wrong:** Saturating arithmetic is correct when clamping is the desired outcome — e.g., capping a counter at `u128::MAX`, decrementing a non-critical metric toward zero, or computing a fee floor. Only flag `saturating_*` when: (a) the operation is on a **balance or share calculation** where underflow/overflow should be an error, AND (b) there is no preceding guard (`ensure!`, `if` check, or explicit comment) validating the operands. If the code has a comment like `// saturating is intentional here` or a preceding bounds check, drop it.

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

## FP-008: Division-before-multiply in fixed-point math libraries

**Pattern:** Flagging `(a / b) * c` ordering in code that uses fixed-point or high-precision math libraries (`FixedU128`, `rug`, `hydra-dx-math`).

**Why it's wrong:** Fixed-point math libraries handle intermediate precision internally — division before multiplication is often correct within these abstractions because the library upscales internally. Only flag division-before-multiply when: (a) the operation uses raw integer types (`u128`, `Balance`) without fixed-point wrappers, AND (b) the denominator can exceed the numerator, causing truncation to zero.

---

## FP-009: Storage writes without deposit for bounded/ephemeral state

**Pattern:** Flagging storage writes that lack a storage deposit when the storage is bounded or ephemeral.

**Why it's wrong:** Not all storage writes need deposits. Bounded collections (`BoundedVec`, `BoundedBTreeMap`) with tight limits, per-block transient state (cleared in `on_finalize`), and storage that requires an existing deposit (e.g., account with ED) to exist are not vulnerable to unbounded bloat. Only flag missing deposits when: (a) the storage is **unbounded or has a high limit** (>1000 entries), AND (b) any signed user can trigger the write, AND (c) there is no deposit, fee, or ED requirement gating the write.

---

## FP-010: Panicking operations with prior validation

**Pattern:** Flagging `.expect("reason; qed")` or `assert!` where a preceding check guarantees the condition holds.

**Why it's wrong:** Substrate convention uses `.expect("description; qed")` (quod erat demonstrandum) to document mathematically or logically proven invariants. If the code has a preceding `ensure!`, `if`/`match` guard, or the value was just validated in the same scope, the `expect` is a defensive assertion, not a vulnerability. Only flag `expect`/`unwrap` when: (a) the "proof" comment is missing or unconvincing, OR (b) the preceding guard doesn't actually cover the case, OR (c) it's on user-supplied input without prior validation.

---

## FP-011: Missing slippage protection on privileged/internal operations

**Pattern:** Flagging governance or protocol-internal operations for missing slippage parameters (min_amount_out, max_price).

**Why it's wrong:** Slippage protection is a user-facing defense against MEV. Governance operations (executed via council/referendum) and protocol-internal operations (called from hooks, not user-initiated) are not sandwichable in the traditional sense. Only flag missing slippage when: (a) the function is callable by **any signed user**, OR (b) the privileged operation can be frontrun by manipulating pool state before the governance extrinsic executes in the block.

---

## FP-012: Stale oracle data on re-added assets (if reset logic exists)

**Pattern:** Flagging stale oracle state after asset removal and re-addition.

**Why it's wrong (when fixed):** If the runtime has been updated to clear oracle entries on `remove_token()` or reinitialize oracle state on `add_token()`, this is a known-fixed issue. Check for oracle accumulator cleanup or equivalent in the remove/add flow before reporting.

---

## FP-013: ExistentialDeposit "too low" without economic analysis

**Pattern:** Flagging a specific ED value as insufficient without calculating the actual cost of an attack.

**Why it's wrong:** ED adequacy depends on token price, storage costs, and the specific chain's economics. An ED of 1_000_000 (1e6) may be trivial for a low-value token or meaningful for an expensive one. Only flag ED as insufficient when you can demonstrate: (a) the cost to create N accounts at the current ED, AND (b) N is enough to meaningfully degrade chain performance, AND (c) the cost is economically trivial relative to the attack impact.
