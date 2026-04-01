# Attack Vectors — Substrate & Rust Common Vulnerability Patterns

> Sources: Distilled from 30 audit reports and 4 bug bounty disclosures across the Polkadot ecosystem (2024–2025). Covers DeFi protocol patterns, access control, cryptographic verification, and state management antipatterns.

---

## 1. Self-Transfer Share/Reward Duplication

**Description:** Transfer functions that don't check `sender == receiver` allow self-transfers that double shares or rewards. The function adds to the receiver (self) without deducting from the sender (also self).

**What to look for:**
- `transfer_share_and_rewards`, `transfer_shares`, or similar functions
- Missing `ensure!(sender != receiver, ...)` check
- Any transfer logic where add and subtract operations on the same account don't cancel out correctly

**Mitigation:** Always validate `sender != receiver` in transfer functions. Add explicit self-transfer tests.

**Seen in:** Acala

---

## 2. Reward Inflation via Rounding on Small Transfers

**Description:** Transferring small fractions of shares causes reward debt to round down to zero. Receiver gets shares with zero debt and can claim already-distributed rewards. Repeated small transfers drain the reward pool.

**What to look for:**
- `reward_debt_transfer = shares_transferred * accumulated_per_share / total` rounding to zero
- Share transfer functions without minimum transfer amount
- Reward pools where small share transfers are cheaper than the claimable rewards

**Mitigation:** Enforce minimum transfer amounts. Round reward debt transfer UP (against the receiver). Consider snapshotting reward state on transfer.

**Seen in:** Acala

---

## 3. Incentive Sandwich (Deposit-Claim-Withdraw)

**Description:** Missing unbonding/lockup period on staking/farming deposits allows attackers to deposit just before rewards accumulate (`on_initialize`), claim rewards, then withdraw immediately — capturing disproportionate rewards.

**What to look for:**
- Reward distribution via `on_initialize` or `accumulate_incentives` without time-weighted shares
- Deposit + claim + withdraw possible within same block or consecutive blocks
- No minimum staking duration or unbonding period

**Mitigation:** Implement unbonding periods. Use time-weighted share calculations for reward distribution. Prevent same-block deposit-and-claim.

**Seen in:** Acala

---

## 4. Confused Deputy / Missing Input Cross-Validation

**Description:** User-supplied parameters that derive security-critical identifiers are not validated against stored authoritative values. Attacker provides a different pool/asset/account identifier than what their position is actually associated with.

**What to look for:**
- Functions accepting user-supplied `pool_id`, `AssetPair`, or similar identifiers used to look up resources
- No cross-check between the user-supplied identifier and the deposit/position's stored association
- Derived values (e.g., `amm_pool_id` from `AssetPair`) not verified against the stored original

**Mitigation:** Always validate user-supplied resource identifiers against stored authoritative data. Don't trust user input to select which resource to operate on.

**Seen in:** Hydration (XYK liquidity mining — Critical, ~$200K)

---

## 5. Burn-Before-Confirm Pattern

**Description:** Executing an irreversible destructive action (burning tokens, deleting state) before confirming the corresponding constructive action (depositing, creating) will succeed. If the second step fails, the first is unrecoverable.

**What to look for:**
- `burn()` followed by `can_deposit()` or `transfer()` that can fail
- State deletion before replacement state is confirmed writable
- Any sequence where step 1 is irreversible and step 2 is fallible

**Mitigation:** Validate all preconditions before any destructive action. Use transactional wrappers. Check `can_deposit` / `can_transfer` before `burn`.

**Seen in:** KILT (bonding curve — permanent fund loss)

---

## 6. Mutable Asset Metadata Corrupting Live State

**Description:** Allowing changes to asset properties (decimals, name, symbol) after the asset is used in pools or other state-dependent systems. Changing decimals retroactively reinterprets all stored balances.

**What to look for:**
- `update_asset` or `set_metadata` functions that can change `decimals`
- Pool math that uses `normalize_value()` on some paths but not all
- Stored balances interpreted differently after metadata change

**Mitigation:** Freeze decimals after first use in pools. If metadata changes are necessary, require migration of all dependent state.

**Seen in:** Hydration (OAK — Major)

---

## 7. Staking Lock/Unlock State Tracking Mismatch

**Description:** Unlock/relock logic tracks chunks independently from actual on-chain balances. Users can unlock tokens (marking them as free balance), transfer them away, then `relock` the already-transferred tokens — effectively staking nonexistent funds.

**What to look for:**
- `unlock_chunks` or `unlocking` storage that isn't reconciled against `free_balance`
- `relock_unlocking` functions that don't verify current balance covers the chunk amount
- Staking systems where unlock + transfer + relock is possible

**Mitigation:** Verify actual free balance before relocking. Use currency locks (`set_lock` / `extend_lock`) that prevent transfer while locked.

**Seen in:** Astar dApp Staking v3 (Critical — double spending)

---

## 8. Retroactive Parameter Changes Affecting Existing Positions

**Description:** Collators/validators/operators can change economic terms (commission rates, fees, pricing) that retroactively affect existing delegators/users who are locked in by unbonding periods.

**What to look for:**
- `setCommission()` or similar parameter changes without time-locks or rate limits
- Delegators locked by `StakeDuration` while operator changes terms
- Missing maximum change limits per period

**Mitigation:** Implement change rate limits (e.g., max 5% per era). Apply time-locks that cover at least one unbonding period. Notify delegators before changes take effect.

**Seen in:** Peaq

---

## 9. Silent Cryptographic Fallback to Identity/Default

**Description:** Invalid cryptographic inputs (malformed points, invalid keys) silently substituted with identity/default values instead of returning errors. In threshold schemes, identity elements are additive identities — they effectively "don't count."

**What to look for:**
- `unwrap_or_else(identity)` or `unwrap_or_default()` on cryptographic point decompression
- Error handling that returns a valid-but-meaningless value for invalid crypto inputs
- Threshold signature schemes where identity element bypasses participation requirement

**Mitigation:** Return errors on invalid cryptographic inputs. Never substitute identity/default values for failed decompression.

**Seen in:** Frontier (Curve25519 precompiles)

---

## 10. Incomplete Cryptographic Verification

**Description:** Custom implementations of cryptographic verification (RSA, WebAuthn, certificate chains) that skip standard checks — padding validation, encoding format, certificate chain traversal.

**What to look for:**
- RSA verification checking only trailing hash bytes, ignoring PKCS#1 v1.5 padding
- WebAuthn verification missing `origin`, `rpIdHash`, or attestation format checks
- Certificate chain validation that checks signatures but not expiry, revocation, or trust anchors

**Mitigation:** Use well-audited cryptographic libraries. Follow specifications completely (RFC 8017 for RSA, WebAuthn spec for attestation). Never implement custom crypto verification unless absolutely necessary.

**Seen in:** Acurast (RSA), Virto (WebAuthn)

---

## 11. Stub/Placeholder Validation in Production

**Description:** Validation functions that `return true` / `return Ok(())` left in production code. Often from development stubs or commented-out implementations.

**What to look for:**
- `fn is_valid() -> bool { true }` or similar always-passing validators
- Functions with bodies entirely commented out, returning `Ok(())`
- `// TODO: implement` in validation paths

**Mitigation:** CI checks for stub patterns. Code review checklist for validation completeness. Never merge commented-out validation logic.

**Seen in:** Virto (device attestation), Acurast (scheduling window)

---

## 12. Post-Arithmetic Input Validation

**Description:** Performing unchecked arithmetic on user-supplied values before validating them. If the arithmetic overflows, the validation never executes.

**What to look for:**
- Arithmetic operations on user input before `ensure!` / bounds checks
- Functions where validation is at the end, after computation
- `let result = input * factor; ensure!(result < max, ...)`  where the multiplication can overflow

**Mitigation:** Validate inputs BEFORE any arithmetic. Use checked arithmetic on user-supplied values.

**Seen in:** Acala (ORML rewards — node crash via overflow)

---

## 13. One-Sided Time Bound Validation

**Description:** Time-sensitive operations validated against only upper or lower bounds. Missing the other bound allows premature or belated action.

**What to look for:**
- Report/claim functions checking `now < max_end_time` without `now > min_start_time`
- Challenge/veto functions without checking if the challenge window is still open
- Expiry checks without checking if the item is not-yet-active

**Mitigation:** Always validate both bounds of time-sensitive windows. Check `start <= now <= end` for any time-bounded operation.

**Seen in:** Acurast (premature report payouts), Hyperbridge (post-deadline veto)

---

## 14. Validate-Once-Trust-Forever

**Description:** Time-bound credentials (attestations, certificates, sessions) validated only at creation/submission but not re-validated on subsequent use. Expired credentials continue to be accepted.

**What to look for:**
- `validate_and_store()` checks expiry but subsequent `check_attestation()` only checks revocation
- Session tokens without periodic re-validation
- Certificates stored once and trusted indefinitely without TTL enforcement

**Mitigation:** Re-validate expiry on every use, not just on submission. Store and check `valid_until` timestamps.

**Seen in:** Acurast

---

## 15. Inconsistent Storage Key Construction

**Description:** Write and delete paths for the same storage use different key construction, causing deletes to miss the target entry. Storage grows unboundedly as entries are never cleaned up.

**What to look for:**
- Write path uses composite key `(recipient, hash(sender, nonce))` but delete uses `(sender, nonce)`
- Storage map key construction that differs between insertion and removal functions
- Lookup/index maps with mismatched key derivation across CRUD operations

**Mitigation:** Use a single key-construction helper shared by all paths. Add tests that verify write-then-delete roundtrips.

**Seen in:** Acurast

---

## 16. Inconsistent Resource Cleanup Across Code Paths

**Description:** Multiple code paths handle resource cleanup (assignment removal, capacity unlock, position closing) but some paths skip releasing locked resources. Over time, this leaks capacity and degrades system availability.

**What to look for:**
- Multiple functions that clean up the same resource type (e.g., `do_cleanup_assignment`, `cleanup_storage`, `finalize_job`)
- Some paths calling `unlock()` while others only remove the index entry
- Locked capacity/collateral tracked separately from the entity it's associated with

**Mitigation:** Centralize cleanup logic in a single function. Add invariant tests that verify all locked resources are eventually released.

**Seen in:** Acurast

---

## 17. Missing Uniqueness Validation on Array Inputs

**Description:** Functions accepting arrays of identifiers (asset IDs, trait IDs, token IDs) without checking for duplicates. One item counted multiple times bypasses diversity requirements.

**What to look for:**
- Minting functions requiring N distinct traits but not deduplicating the input array
- Pool creation accepting duplicate asset IDs
- Batch operations where the same ID processed twice has unintended effects

**Mitigation:** Deduplicate input arrays or validate uniqueness. Use `BTreeSet` for collected identifiers.

**Seen in:** Peaq (NFT traits), Hydration (Stableswap pool creation)

---

## 18. Authorization Role Check Without Membership Verification

**Description:** Access control that checks a role type exists but doesn't verify the caller actually holds that role. `ensure_signed` is used where role-specific verification is needed.

**What to look for:**
- `match role { CircuitRole::Executor => ensure_signed(origin)?, ... }` — checks signed but not executor
- Role-based dispatch that accepts any signed origin regardless of role assignment
- `is_owner()` checking group membership instead of specific resource ownership

**Mitigation:** Verify role membership, not just signature. Use `T::RoleOrigin::ensure_origin(origin)` or explicit role storage checks.

**Seen in:** t3rn (any user impersonates any role), Peaq (DID attribute ownership)

---

## 19. Session Key Hijacking via Public Exposure

**Description:** Session keys exposed through events or storage queries can be stolen by attackers who call `add_session_key` with the stolen key, removing it from the original owner.

**What to look for:**
- Session key creation emitting the key in events
- `add_session_key` that first removes the key from any previous owner
- No "already in use" guard on key registration

**Mitigation:** Check if key is already registered before reassignment. Don't expose active session keys in events. Use key derivation that binds keys to owners.

**Seen in:** Virto

---

## 20. Documentation-Implementation Mismatch on Invariants

**Description:** Documented invariants (e.g., "pricing cannot change while assigned") not enforced in code. The mismatch creates false security assumptions during code review and integration.

**What to look for:**
- Doc comments stating restrictions that aren't reflected in the function body
- `// invariant: X` comments without corresponding `ensure!` checks
- Spec documents describing constraints that don't appear in tests

**Mitigation:** Every documented invariant must have a corresponding code check and test. Use property-based testing to verify invariants hold.

**Seen in:** Acurast (advertisement pricing changes while assigned)
