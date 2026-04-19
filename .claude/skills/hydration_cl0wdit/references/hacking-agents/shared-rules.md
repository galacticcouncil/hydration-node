# Shared Scan Rules

## Reading

Your bundle has two sections:

1. **Core source** (inline) — read in parallel chunks (offset + limit), compute offsets from the line count in your prompt.
2. **Peripheral file manifest** — file paths under `# Peripheral Files (read on demand)`. Read only those relevant to your specialty.

When matching function names, check both public dispatchables (`#[pallet::call]`), hooks (`on_initialize`, `on_finalize`, `on_idle`, `on_runtime_upgrade`), and internal helpers they call.

## Cross-pallet patterns

When you find a bug in one pallet, **weaponize that pattern across every other pallet in the bundle.** Search by function name AND by code pattern. Finding unsafe arithmetic in `PalletA::do_transfer` means you check every other pallet's transfer logic — missing a repeat instance is an audit failure.

After scanning: escalate every finding to its worst exploitable variant (DoS may hide fund theft). Then revisit every function where you found something and attack the other branches.

## Do not report

Admin/governance/sudo functions doing admin things. Standard DeFi tradeoffs (MEV, rounding dust). Self-harm-only bugs. "Governance can rug" without a concrete mechanism. Anything a linter or `cargo clippy` would catch.

## Output

Return structured blocks only — no preamble, no narration. Exception: vector scan agent outputs its classification block first.

FINDINGs have concrete, unguarded, exploitable attack paths. LEADs have real code smells with partial paths — default to LEAD over dropping.

**Every FINDING must have a `proof:` field** — concrete values, traces, or state sequences from the actual code. No proof = LEAD, no exceptions.

**One vulnerability per item.** Same root cause = one item. Different fixes needed = separate items.

```
FINDING | pallet: Name | function: func | bug_class: kebab-tag | group_key: Pallet | function | bug-class
path: caller → extrinsic/hook → internal call → storage mutation → impact
proof: concrete values/trace demonstrating the bug
description: one sentence
fix: one-sentence suggestion

LEAD | pallet: Name | function: func | bug_class: kebab-tag | group_key: Pallet | function | bug-class
code_smells: what you found
description: one sentence explaining trail and what remains unverified
```

The `group_key` enables deduplication: `PalletName | functionName | bug_class`. Agents may add custom fields.
