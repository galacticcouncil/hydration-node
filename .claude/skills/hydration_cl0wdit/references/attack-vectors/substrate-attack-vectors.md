# Attack Vectors — Substrate / Polkadot & Rust Common Vulnerabilities

> Sources:
> - [Common Vulnerabilities in Substrate/Polkadot Development](https://forum.polkadot.network/t/common-vulnerabilities-in-substrate-polkadot-development/3938)
> - [CoinFabrik Scout Audit — Substrate Pallet Detectors](https://coinfabrik.github.io/scout-audit/docs/category/substrate-pallets) and [Rust Detectors](https://coinfabrik.github.io/scout-audit/docs/category/rust)

---

## 1. Unsafe Arithmetic

**Description:** Arithmetic operations without overflow/underflow checks produce incorrect calculations and exploitable inconsistencies. This is especially dangerous in balance calculations, fee computation, and economic logic. In release mode, Rust wraps integer overflow silently by default.

**What to look for:**
- Use of `+`, `-`, `*`, `/` operators on numeric types instead of checked/saturating variants
- Arithmetic in balance transfers, reward distributions, fee calculations
- Division that could result in division-by-zero
- Missing `overflow-checks = true` in `[profile.release]` in Cargo.toml

**Mitigation:** Use `checked_add`, `checked_sub`, `checked_mul`, `checked_div`, or `saturating_add`, `saturating_sub`, etc. Use `ensure!` or `ok_or` to handle `None` from checked operations. Set `overflow-checks = true` in release profile as a safety net.

---

## 2. Saturating Arithmetic Masking Errors

**Description:** `saturating_add()`, `saturating_sub()`, etc. clamp results to type limits instead of failing. A saturating subtraction on a balance can return 0 instead of failing, masking critical errors and enabling silent value creation.

**What to look for:**
- Defaulting to `saturating_*` methods for all arithmetic without explicit justification
- `saturating_sub` on balances where underflow should be an error (e.g., insufficient balance checks)
- Any saturating operation where clamping hides a logic error

**Mitigation:** Prefer `checked_*` methods. Only use saturating arithmetic with explicit justification where clamping is the intended behavior.

---

## 3. Divide Before Multiply

**Description:** Integer division truncates toward zero. Dividing before multiplying loses precision — `(a / b) * c` can yield 0 when `a < b`, even if `a * c / b` would be non-zero.

**What to look for:**
- Performing division before multiplication in multi-step calculations
- Precision loss in fee, reward, or share calculations

**Mitigation:** Reorder to multiply first: `(a * c) / b`. Watch for intermediate overflow when doing so.

---

## 4. Incorrect Exponentiation

**Description:** The `^` operator in Rust is bitwise XOR, not exponentiation. `x ^ 3` computes XOR with 3, not x cubed.

**What to look for:**
- Using `^` for power operations
- Unexpected results from bitwise operations misused as math

**Mitigation:** Use `.pow()` for exponentiation, `.bitxor()` if XOR is intended.

---

## 5. Unsafe Conversion

**Description:** Type conversions without verification — especially downcasting from larger to smaller integer types — introduce silent truncation errors exploitable by attackers.

**What to look for:**
- Direct `as` casts between numeric types (e.g., `u128 as u64`)
- Conversions between `Balance`, `BlockNumber`, and other runtime types without bounds checking
- Use of `.into()` where the conversion could lose precision

**Mitigation:** Use `unique_saturated_into`, `saturated_into`, or `TryInto` with error handling instead of raw `as` casts.

---

## 6. Panicking Operations in Runtime Code

**Description:** `panic!`, `assert!`, `.unwrap()`, and `.expect()` halt execution mid-transaction, potentially leaving state inconsistent. In on-chain runtime code this can brick block production.

**What to look for:**
- `assert!()` instead of returning `DispatchResult` errors
- `panic!()` for error conditions in dispatchable functions
- `.unwrap()` / `.expect()` on storage reads or fallible operations without prior validation
- `.expect("")` with empty diagnostic messages

**Mitigation:** Return `DispatchResult` errors. Use `ok_or(Error::<T>::...)?.into()`, `unwrap_or_default()`, or match/if-let patterns. Provide descriptive messages when `.expect()` is justified.

---

## 7. Insecure Randomness

**Description:** Weak random number generation compromises features like lotteries, voting, or any logic dependent on unpredictability. Substrate's `Randomness Collective Flip` pallet generates low-influence random values based on block hashes from the previous 81 blocks.

**What to look for:**
- Usage of `RandomnessCollectiveFlip` or raw block hash-based randomness
- Any on-chain randomness used to decide economic outcomes (rewards, selection, shuffling)

**Mitigation:** Use VRF (Verifiable Random Function) methods (e.g., BABE's VRF output) or custom trusted oracles for randomness.

---

## 8. Storage Exhaustion

**Description:** Insufficient deposit or fee mechanisms for storage access allow attackers to bloat on-chain storage, degrading performance and increasing operational costs.

**What to look for:**
- Storage maps or vectors that grow unboundedly without requiring a deposit
- Missing `StorageDepositPerItem` / `StorageDepositPerByte` enforcement
- Extrinsics that write to storage without proportional fee/deposit charges

**Mitigation:** Require adequate deposits for storage writes. Enforce bounded storage collections (`BoundedVec`, `BoundedBTreeMap`).

---

## 9. Unbounded Decoding

**Description:** Lack of depth limits when decoding nested objects (e.g., deeply nested `Call` enums in batch extrinsics) can trigger stack overflows, potentially preventing validators from producing new blocks.

**What to look for:**
- Use of `.decode()` on user-supplied input without depth limits
- Nested `Call` types (especially in utility/multisig/proxy pallets)

**Mitigation:** Use `decode_with_depth_limit` instead of plain `decode`. Substrate ≥ v0.9.37 includes this by default for extrinsic decoding.

---

## 10. Invalid Extrinsic Weight

**Description:** Inaccurate weight calculations that fail to account for worst-case execution paths allow attackers to spam the network with under-priced extrinsics, causing slowdowns.

**What to look for:**
- Benchmarks that don't cover maximum input sizes or worst-case DB reads/writes
- Hard-coded weights instead of benchmark-derived weights
- Missing benchmarks for extrinsics with variable-length inputs (e.g., `Vec<T>`)
- Reusing weight functions across different extrinsics

**Mitigation:** Benchmark every extrinsic with worst-case parameters. Use `frame_benchmarking` to generate accurate weights unique to each extrinsic.

---

## 11. XCM Arbitrary Execution

**Description:** Permissive XCM (Cross-Consensus Messaging) filters allow unauthorized cross-chain calls. Using `Everything` as a call filter instead of restrictive `SafeCallFilter` configurations creates attack surfaces.

**What to look for:**
- `type CallFilter = Everything` or overly broad XCM call filters
- XCM `Transact` instructions accepted from untrusted origins
- Missing origin validation on XCM-triggered calls

**Mitigation:** Use restrictive `SafeCallFilter` configurations. Whitelist only the specific calls that should be executable via XCM.

---

## 12. XCM Denial of Service (DoS)

**Description:** Inadequate XCM message filtering enables attackers to flood sibling chains with messages, potentially overwhelming message queues or halting block processing.

**What to look for:**
- Missing rate limits or fee requirements for outbound XCM messages
- Accepting XCM messages from untrusted or unverified origins
- No bounds on XCM message queue depth

**Mitigation:** Implement proper filtering, trust boundaries, and rate limiting for XCM messages. Charge adequate fees for cross-chain operations.

---

## 13. Replay Issues

**Description:** Improper nonce handling or missing replay protection allows transactions to be reused — either across chains (cross-chain replay) or within the same chain.

**What to look for:**
- Custom `SignedExtension` / `TransactionExtension` implementations that skip nonce checks
- Extrinsics or messages that lack unique identifiers
- Off-chain signed messages without chain ID or nonce binding

**Mitigation:** Ensure nonce validation occurs in the State Transition Function. Include chain ID and nonce in all signed payloads.

---

## 14. Batch Processing DoS

**Description:** Invalid items in batch operations cause entire batch failures when execution stops on the first error instead of skipping problematic items. Malicious actors can inject invalid storage items to disrupt batch processing.

**What to look for:**
- Iterating over storage and processing items where one invalid item aborts the entire operation
- `for item in items { process(item)?; }` patterns where `?` propagates errors from user-controlled data
- Hooks (e.g., `on_initialize`, `on_idle`) that iterate storage without defensive handling

**Mitigation:** Use defensive iteration: skip or log invalid items instead of aborting. Consider `force_` variants for admin recovery.

---

## 15. Missing Zero Check

**Description:** Accepting zero-value Balance parameters causes unnecessary storage writes, event emissions, and wasted computation.

**What to look for:**
- Functions that process Balance/Amount parameters without checking for zero

**Mitigation:** Add early return: `ensure!(amount > Zero::zero(), Error::<T>::ZeroAmount)`.

---

## 16. Unsigned Extrinsic Risk

**Description:** Unsigned extrinsics have no associated fee or signature, allowing attackers to submit transactions at zero cost — enabling transaction pool flooding and DoS.

**What to look for:**
- Using `ensure_none(origin)?` in dispatchable functions without additional validation
- Missing or weak `ValidateUnsigned` implementation

**Mitigation:** Use `ensure_signed(origin)?` unless unsigned submission is specifically required. If unsigned is needed, implement strict `ValidateUnsigned` checks with rate limiting.

---

## 17. Unsafe Code Blocks

**Description:** `unsafe` blocks bypass Rust's memory safety guarantees, enabling undefined behavior, buffer overflows, and use-after-free.

**What to look for:**
- Using `unsafe {}` for pointer manipulation or FFI when safe alternatives exist

**Mitigation:** Use safe Rust APIs. Only use `unsafe` when absolutely necessary (e.g., FFI boundaries) with thorough documentation and review.

---

## 18. Outdated Crates / Known Vulnerabilities

**Description:** Using older versions of Substrate/Polkadot dependencies introduces known vulnerabilities, missing security patches, and compatibility problems.

**What to look for:**
- Pinned dependency versions significantly behind the latest release
- Mixed versions of Substrate crates within the same runtime
- Not auditing or updating dependencies regularly

**Mitigation:** Keep dependencies up to date. Use consistent versions. Run `cargo audit` regularly. Monitor RustSec advisory database.

---

## 19. Insufficient Logging / Observability

**Description:** Insufficient logging in critical pallet components hampers diagnostics during failures, halts, or exploit attempts. Conversely, debug logging macros in production increase execution weight and can leak information.

**What to look for:**
- Critical error paths that silently return errors without logging
- No defensive logging around storage migrations
- `debug!()`, `info!()` left in production pallet code

**Mitigation:** Add `log::warn!` / `log::error!` at critical decision points. Use `deposit_event()` for observable state changes. Remove or gate debug logging behind feature flags.

---

## 20. Generic DispatchError

**Description:** Generic error strings are hard to match on, debug, and monitor. Callers can't programmatically distinguish error types.

**What to look for:**
- Returning `Err(DispatchError::Other("some message"))`

**Mitigation:** Define specific error variants in the pallet's `#[pallet::error]` enum and return those.
