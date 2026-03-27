# Report Formatting

## Report Path

When `--file-output` is set, resolve the git repository root via `git rev-parse --show-toplevel` and save the report to `{repo_root}/findings/{timestamp}-hydration-{feature-name}-cl0wdit.md`, where `{feature-name}` is the name of the feature or pallet and `{timestamp}` is `YYYYMMDD-HHMMSS` at scan time. Create the `findings/` directory if it doesn't exist.

## Output Format

````
# Security Audit — <Runtime / Pallet name or repo name>

---

## Scope

| Field | Value |
|---|---|
| **Mode** | ALL / default / filename |
| **In-scope files** | `pallet_foo/src/lib.rs` · `pallet_bar/src/lib.rs`<br>`runtime/src/lib.rs` | <!-- list every file, 2-3 per line -->
| **Confidence threshold (1–100)** | N |

---

## Findings

[95] **1. <Title>**

`pallet_name::dispatchable_or_hook` · Confidence: 95

**Description**
<One sentence: what the vulnerable pattern is and how it can be exploited.>

**Fix**

```diff
- vulnerable line(s)
+ fixed line(s)
```

---

[82] **2. <Title>**

`pallet_name::function_name` · Confidence: 82

**Description**
<One sentence: what the vulnerable pattern is and how it can be exploited.>

**Fix**

```diff
- vulnerable line(s)
+ fixed line(s)
```

---

< ... remaining findings ... >

---

## Summary

| # | Confidence | Title |
|---|---|---|
| 1 | [95] | <title> |
| 2 | [82] | <title> |
| | | **— Below Confidence Threshold —** |
| 3 | [72] | <title> |
| 4 | [55] | <title> |

---

> **Disclaimer:** This audit was conducted by an AI agent. Automated analysis cannot guarantee the absence of vulnerabilities. Independent manual review, formal verification where applicable, and runtime monitoring are strongly recommended before deploying to production.

````

## Rules

- Follow the template above exactly.
- Sort findings highest confidence first.
- Location format: use `pallet_name::function_name` (Rust path style), not dot notation.
- Findings below the confidence threshold receive a description but **no Fix block**.
- Draft findings directly into the report format — do not regenerate or reword them after the fact.
