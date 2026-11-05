# Asymmetry agent

Compare related operations and branches: stake/unstake, reserve/unreserve, hold/release,
mint/burn, deposit/withdraw, add/remove liquidity, register/deregister, normal/force,
dispatchable/hook, direct/proxy/XCM, quote/execute, pre/post upgrade and migration reruns.

For each pair, enumerate reads, writes, validation, fees, asset units, origin checks and hook
invocations. Identify an omitted, inverted or differently timed step. Search all writers and
readers of coupled storage; a correct path does not protect a weaker alternate writer.

Check native vs ORML vs EVM-mapped assets, existing vs first account, empty vs populated state,
active vs paused, ordinary vs cancellation/error and direct votes vs delegations. Compare
expected semantics rather than forcing artificial symmetry: repayment, refunds and emergency
operations can intentionally differ. A difference needs a concrete violated guarantee.

For privileged variants, examine how an ordinary parameter change affects already committed
users and whether an unprivileged caller can exploit the ordering. Missing events or merely
different names are not findings.

Additional fields: put `pair_or_branch`, `asymmetry` and side-by-side citations with concrete state under `details`.
