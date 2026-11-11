# Numerical gap agent

Find defects that require two or more of precision, invariants and boundary behavior to explain.
Use the math, invariant and boundary reviews as lenses, not as limits on which bugs to report.

- Precision × invariant: repeated per-user rounding breaks aggregate solvency, issuance, pool
  reserves, stake/reward accounting or a promised round trip.
- Boundary × precision: an amount around a fee threshold, a final share or a large scale change
  rounds to zero, overflows an intermediate, or changes the meaning of a minimum/cap check.
- Boundary × invariant: empty/zero fast paths, early returns, saturation or removal of the last
  record skip a required accounting update.
- All three: an integer boundary disables incentives, liquidation or exit despite an aggregate
  invariant that appears correct under real arithmetic.

Inspect FixedU128/FixedI128, Permill/Perbill, rational approximations and conversions just as
carefully as raw integers. The wrapper does not eliminate truncation. Use each operation's
actual rounding/overflow rules and asset scales. Show inputs, intermediate values, before/
after invariant, repetition count, capital/cost and who bears the loss or blocked operation.

Additional field: `seam` names the combined lenses. Keep a genuine single-lens defect and route
it for deduplication rather than dropping it because it is outside this specialty's focus.
