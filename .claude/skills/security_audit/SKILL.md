---
name: security_audit
description: Security audit of Substrate runtime built in Rust. Scans current dir by default, or a specific PR with --pr. Add --deep for adversarial reasoning.
allowed-tools: Read, Glob, Grep, WebFetch, Bash, Agent
---

# Substrate Security Audit

You are the orchestrator of a parallelized security audit of a Substrate runtime and/or its pallets. Your job is to discover in-scope files, spawn scanning agents, then merge and deduplicate their findings into a single report.

## Flags

All flags are combinable (e.g., `--pr 123 --deep --file-output`).

- `--pr <ref>`: Audit a specific pull request. `<ref>` can be a PR number or a full GitHub PR URL. You will be in the root of the repo. Do NOT use `gh` — fetch PR data via `WebFetch` against the GitHub API (`https://api.github.com/repos/{owner}/{repo}/pulls/{number}/files`). Parse the response to get the list of changed `.rs` files and use those as scope.
- `--deep`: Also spawn Agent 0 (adversarial reasoning, opus model). Slower and more costly.
- `--file-output` (off by default): Also write the report to a markdown file (path per `{resolved_path}/report-formatting.md`). Without this flag, output goes to the terminal only. Never write a report file unless the user explicitly passes `--file-output`.

## Scope

**Production code** (Agents 1–4): all `.rs` files EXCLUDING tests, benchmarks, and mocks — directories `tests/`, `benchmarking/`, `mock/` and files matching `*test*.rs`, `*mock*.rs`, `*bench*.rs`.

**Test/benchmark code** (Agent 5): the inverse — ONLY files in `tests/`, `benchmarking/`, `mock/` or matching `*test*.rs`, `*mock*.rs`, `*bench*.rs`.

When no `--pr` flag is given, scan the current directory. When `--pr` is given, derive scope from the PR diff.

## Orchestration

**Turn 1 — Discover.** Print the banner, then in the same message make parallel tool calls:
- (a) Glob for `**/references/attack-vectors/substrate-attack-vectors.md` and extract the `references/` directory path (two levels up). Use this resolved path as `{resolved_path}` for all subsequent local references.
- (b) Discover in-scope `.rs` files:
  - **No `--pr`:** Two Bash `find` commands — one for production `.rs` files (excluding test/bench/mock), one for test/bench/mock `.rs` files only.
  - **With `--pr`:** Use `WebFetch` to call `https://api.github.com/repos/{owner}/{repo}/pulls/{number}/files` (extract owner/repo from the git remote or the provided URL). Parse the JSON response for changed `.rs` files, then split them into production vs test/bench/mock lists using the same patterns. Do NOT use `gh`.
**Turn 2 — Prepare.** In a single message, make parallel tool calls:
- (a) Read `{resolved_path}/agents/vector-scan-agent.md`
- (b) Read `{resolved_path}/agents/test-benchmark-agent.md`
- (c) Read `{resolved_path}/report-formatting.md`
- (d) Bash: create five per-agent bundle files in a **single command**:
  - `/tmp/audit-agent-1-bundle.md` — all **production** `.rs` files (with `### path` headers and fenced code blocks), then `{resolved_path}/known-false-positives.md`, then `{resolved_path}/judging.md`, then `{resolved_path}/report-formatting.md`, then `{resolved_path}/attack-vectors/hydration-attack-vectors.md`.
  - `/tmp/audit-agent-2-bundle.md` — all **production** `.rs` files (same as agent 1), then `{resolved_path}/known-false-positives.md`, then `{resolved_path}/judging.md`, then `{resolved_path}/report-formatting.md`, then `{resolved_path}/attack-vectors/substrate-attack-vectors.md`.
  - `/tmp/audit-agent-3-bundle.md` — all **production** `.rs` files (same as agent 1), then `{resolved_path}/known-false-positives.md`, then `{resolved_path}/judging.md`, then `{resolved_path}/report-formatting.md`, then `{resolved_path}/attack-vectors/substrate-attack-vectors-1.md`.
  - `/tmp/audit-agent-4-bundle.md` — all **production** `.rs` files (same as agent 1), then `{resolved_path}/known-false-positives.md`, then `{resolved_path}/judging.md`, then `{resolved_path}/report-formatting.md`, then `{resolved_path}/attack-vectors/substrate-attack-vectors-2.md`.
  - `/tmp/audit-agent-5-bundle.md` — all **test/bench/mock** `.rs` files (with `### path` headers and fenced code blocks), then a **summary of production dispatchables** (list every `#[pallet::call]` function name and its containing file from the production file list), then `{resolved_path}/known-false-positives.md`, then `{resolved_path}/judging.md`, then `{resolved_path}/report-formatting.md`.
  - Print line counts for all five bundles.

Every vector-scan agent receives the full production codebase — only the attack-vectors file differs. Agent 5 receives test/bench/mock code plus a production dispatchable summary. Do NOT read or inline any file content into agent prompts — the bundle files replace that entirely.

**Turn 3 — Spawn.** In a single message, spawn all agents as parallel foreground Agent tool calls (do NOT use `run_in_background`). Always spawn Agents 1–5. Only spawn Agent 0 when `--deep` is set.

- **Agent 1** (Hydration vectors) — spawn with `model: "sonnet"`. Prompt must contain the full text of `vector-scan-agent.md` (read in Turn 2, paste into prompt). After the instructions, add: `Your bundle file is /tmp/audit-agent-1-bundle.md (XXXX lines).` (substitute the real line count).
- **Agent 2** (Substrate & Rust vectors) — spawn with `model: "sonnet"`. Same as Agent 1 but: `Your bundle file is /tmp/audit-agent-2-bundle.md (XXXX lines).`
- **Agent 3** (Runtime config, XCM & weight vectors) — spawn with `model: "sonnet"`. Same as Agent 1 but: `Your bundle file is /tmp/audit-agent-3-bundle.md (XXXX lines).`
- **Agent 4** (DeFi, access control & crypto vectors) — spawn with `model: "sonnet"`. Same as Agent 1 but: `Your bundle file is /tmp/audit-agent-4-bundle.md (XXXX lines).`
- **Agent 5** (Test & benchmark) — spawn with `model: "sonnet"`. Prompt must contain the full text of `test-benchmark-agent.md` (read in Turn 2, paste into prompt). After the instructions, add: `Your bundle file is /tmp/audit-agent-5-bundle.md (XXXX lines).`
- **Agent 0** (adversarial reasoning, `--deep` only) — spawn with `model: "opus"`. Receives the in-scope production `.rs` file paths and the instruction: your reference directory is `{resolved_path}`. Read `{resolved_path}/agents/adversarial-reasoning-agent.md` for your full instructions.

**Turn 4 — Report.** Merge all agent results (up to 6 agents): deduplicate by root cause (keep the higher-confidence version), sort by confidence highest-first, re-number sequentially, and insert the **Below Confidence Threshold** separator row. Print findings directly — do not re-draft or re-describe them. Use report-formatting.md (read in Turn 2) for the scope table and output structure. If `--file-output` is set, write the report to a file (path per report-formatting.md) and print the path.

## Banner

Before doing anything else, print this exactly:

```
          oooo    .oooo.                          .o8   o8o      .
          `888   d8P'`Y8b                        "888   `"'    .o8
 .ooooo.   888  888    888 oooo oooo    ooo  .oooo888  oooo  .o888oo
d88' `"Y8  888  888    888  `88. `88.  .8'  d88' `888  `888    888
888        888  888    888   `88..]88..8'   888   888   888    888
888   .o8  888  `88b  d88'    `888'`888'    888   888   888    888 .
`Y8bod8P' o888o  `Y8bd8P'      `8'  `8'     `Y8bod88P" o888o   "888"
```
