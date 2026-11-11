# Senior auditor reasoning

Adapted from Pashov's three reasoning tools: explain the behavior, question assumptions, and
invert the intended result. Apply them to concrete Substrate execution, then test the claim
against the strongest counterexample. These are reasoning tools, not extra report sections.

- Explain a new entry point in plain terms: who can call it, what value/state changes, what
  can fail, and who can recover. A function name or trait bound is not a behavioral proof.
- Ask what each check assumes. Resolve the actual runtime implementation, feature flags,
  storage layout and configured limits. A generic trait call does not imply ERC20 behavior,
  and a wrapper does not prove that a storage transaction spans its callers.
- Invert each promised result. If users can exit, construct a state that prevents exit. If an
  upgrade preserves balances, test cancellation, expiry, delegation and malformed state. If
  work is bounded, inspect how the bound is reached and how each iteration advances.
- Trace a candidate with concrete amounts, origins, state keys and block/time units. Look for
  a worse supported consequence and repeated instances, but also read the guard that could
  refute it. A correct refutation changes the verdict; discomfort alone does not confirm it.
- Distinguish atomicity from availability. Rolling back writes after an `Err` is different
  from trapping the runtime before a block can complete. Trace what a later block or upgrade
  can actually do to recover. Check accounting for work that is rolled back.
- Check real storage access behavior, including overlay deletions, cursors, prefix clears,
  decoding bounds and proof size. An output-correct unit test does not establish execution
  cost or worst-case upgrade safety. Test representative persisted state when relevant.

Do not claim a test, live state, model identity, or safety property you did not verify.
