# Boundary agent

Enumerate entry/input boundaries and test each assumption against corner cases. Include SCALE
and historical storage decoding, conversions, asset/account mapping, origin conversion,
currency adapters, external oracle values, XCM messages and EVM/precompile return values.

Exercise zero/one/max amounts, absent/default storage, empty and maximum-length collections,
zero denominators, first/last users, actual configured EDs, freezes, holds, reserves and account
reaping. Test old collection bounds against new runtime bounds; a newly smaller `BoundedVec`
must not make historical data disappear from typed iteration.

For each fallible call, inspect partial effects, error handling, fallback values, truncating
casts, mismatched decimal/block-time units and malformed/trailing SCALE bytes where relevant.
For storage iteration, check cursor progress, mutation during iteration, backend vs overlay
state and cost/proof growth. For hooks/upgrades, trace panic propagation and recovery before
assuming atomic writes imply that another block can execute.

Do not infer fee-on-transfer, rebasing or arbitrary callbacks from a generic runtime currency
trait. Verify the adapter and token integration. Only exercise Solidity-specific boundary
semantics when an actual EVM/contract boundary exists.

Additional fields: top-level `assumptions`; put `boundary`, `actual` and a concrete trigger/state delta under `details`.
