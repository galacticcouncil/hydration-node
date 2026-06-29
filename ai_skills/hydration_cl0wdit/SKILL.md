---
name: hydration_cl0wdit
description: Security audit of Substrate runtime built in Rust. Scans current dir by default, or a specific PR with --pr.
allowed-tools: Read, Glob, Grep, WebFetch, Bash, Agent
---

# Substrate Security Audit

You are the orchestrator of a parallelized security audit of a Substrate runtime and/or its pallets.

## Codex compatibility

This skill is shared across agent environments. If you are running in Codex, map the Claude-oriented tool names as follows:

- `Read` -> `sed`, `rg`, or other file-read commands.
- `Glob` -> `rg --files` or `find`.
- `Grep` -> `rg`.
- `Bash` -> `exec_command`.
- `WebFetch` -> the web tool, or `curl` only when shell network access is available.
- `Agent` -> Codex sub-agents, following the active Codex environment policy for agent spawning. If sub-agents are unavailable or disallowed in the current environment, run a reduced local review or ask the user how to proceed.

The banner and four-turn orchestration apply only when the user requests an audit run. They do not apply when the user asks to inspect, update, or explain this skill.

## Mode Selection

**Exclude pattern:** skip directories `tests/`, `benchmarking/`, `mock/` and files matching `*test*.rs`, `*mock*.rs`, `*bench*.rs`.

- **Default** (no arguments): scan all `.rs` files using the exclude pattern. Use Bash `find` (not Glob).
- **`$filename ...`**: scan the specified file(s) only.

**Flags:**

- `--pr <ref>`: Audit a specific pull request. `<ref>` can be a PR number or a full GitHub PR URL. Do NOT use `gh` — fetch PR data via `WebFetch` against the GitHub API (`https://api.github.com/repos/{owner}/{repo}/pulls/{number}/files`). Parse the response for changed `.rs` files.
- `--file-output` (off by default): also write the report to a markdown file at the path specified by `{resolved_path}/report-formatting.md`. Never write a report file unless explicitly passed.

## Orchestration

**Turn 1 — Discover.** Print the banner, then make these parallel tool calls in one message:

a. Discover in-scope `.rs` files per mode selection:
   - **No `--pr`:** Two Bash `find` commands — one for production `.rs` files (excluding test/bench/mock), one for test/bench/mock `.rs` files only.
   - **With `--pr`:** Use `WebFetch` to call `https://api.github.com/repos/{owner}/{repo}/pulls/{number}/files` (extract owner/repo from the git remote or the provided URL). Parse the JSON response for changed `.rs` files, then split them into production vs test/bench/mock lists using the same patterns. Do NOT use `gh`.
b. Glob for `**/references/attack-vectors/substrate-attack-vectors.md` — extract the `references/` directory (two levels up) as `{resolved_path}`
c. Read the local `VERSION` file from the same directory as this skill
d. Fetch `https://raw.githubusercontent.com/galacticcouncil/hydration-node/main/ai_skills/hydration_cl0wdit/VERSION` (`Bash curl -sf` in Claude; web tool in Codex when shell network is restricted)
e. Bash `mktemp -d /tmp/audit-XXXXXX` → store as `{bundle_dir}`

If the remote VERSION fetch succeeds and differs from local, print `⚠ hydration_cl0wdit v{local} is outdated — a newer version is available in the repo`. If it fails, skip silently.

**Turn 2 — Prepare.** In one message, make parallel tool calls: (a) Read `{resolved_path}/report-formatting.md`, (b) Read `{resolved_path}/judging.md`.

Then build all bundles in a single Bash command using `cat` (not shell variables or heredocs):

1. `{bundle_dir}/source.md` — ALL in-scope production `.rs` files, each with a `### path` header and fenced code block.
2. Agent bundles = `source.md` + agent-specific files:

| Bundle                | Appended files (relative to `{resolved_path}`)                                                                             |
| --------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `agent-1-bundle.md`   | `attack-vectors/hydration-attack-vectors.md` + `hacking-agents/vector-scan-agent.md` + `hacking-agents/shared-rules.md`    |
| `agent-2-bundle.md`   | `attack-vectors/substrate-attack-vectors.md` + `hacking-agents/vector-scan-agent.md` + `hacking-agents/shared-rules.md`    |
| `agent-3-bundle.md`   | `attack-vectors/substrate-attack-vectors-1.md` + `hacking-agents/vector-scan-agent.md` + `hacking-agents/shared-rules.md`  |
| `agent-4-bundle.md`   | `attack-vectors/substrate-attack-vectors-2.md` + `hacking-agents/vector-scan-agent.md` + `hacking-agents/shared-rules.md`  |
| `agent-5-bundle.md`   | `hacking-agents/math-precision-agent.md` + `hacking-agents/shared-rules.md`                                                |
| `agent-6-bundle.md`   | `hacking-agents/access-control-agent.md` + `hacking-agents/shared-rules.md`                                                |
| `agent-7-bundle.md`   | `hacking-agents/economic-security-agent.md` + `hacking-agents/shared-rules.md`                                             |
| `agent-8-bundle.md`   | `hacking-agents/execution-trace-agent.md` + `hacking-agents/shared-rules.md`                                               |
| `agent-9-bundle.md`   | `hacking-agents/invariant-agent.md` + `hacking-agents/shared-rules.md`                                                     |
| `agent-10-bundle.md`  | `hacking-agents/first-principles-agent.md` + `hacking-agents/shared-rules.md`                                              |
| `agent-11-bundle.md`  | test/bench/mock `.rs` files (with `### path` headers) + production dispatchable summary + `hacking-agents/test-benchmark-agent.md` |

Every hacking agent (1–10) receives the full production codebase via `source.md`. Agent 11 receives test/bench/mock code plus a production dispatchable summary (list every `#[pallet::call]` function name and its containing file). All bundles also get `known-false-positives.md` + `judging.md` + `report-formatting.md` appended.

Print line counts for every bundle and `source.md`. Do NOT inline file content into agent prompts.

**Turn 3 — Spawn.** In one message, spawn all 11 agents as parallel audit workers (Claude: foreground Agent calls; Codex: sub-agents). Prompt template (substitute real values):

```
Your bundle file is {bundle_dir}/agent-N-bundle.md (XXXX lines).
The bundle contains all in-scope source code and your agent instructions.
Read the bundle fully before producing findings.
```

**Turn 4 — Deduplicate, validate & output.** Single-pass: deduplicate all agent results, gate-evaluate, and produce the final report in one turn. Do NOT print an intermediate dedup list — go straight to the report.

1. **Deduplicate.** Parse every FINDING and LEAD from all agents. Group by `group_key` field (format: `Pallet | function | bug-class`). Exact-match first; then merge synonymous bug_class tags sharing the same pallet and function. Keep the best version per group, number sequentially, annotate `[agents: N]`.

   Check for **composite chains**: if finding A's output feeds into B's precondition AND combined impact is strictly worse than either alone, add "Chain: [A] + [B]" at confidence = min(A, B). Most audits have 0–2.

2. **Gate evaluation.** Run each deduplicated finding through the four gates in `judging.md` (do not skip or reorder). Evaluate each finding exactly once — do not revisit after verdict.

   **Single-pass protocol:** evaluate every relevant code path ONCE in fixed order (hooks → dispatchables → internal helpers → cross-pallet calls). One-line verdict per path: `BLOCKS`, `ALLOWS`, `IRRELEVANT`, or `UNCERTAIN`. Commit after all paths — do not re-examine. `UNCERTAIN` = `ALLOWS`.

3. **Lead promotion & rejection guardrails.**
   - Promote LEAD → FINDING (confidence 75) if: complete exploit chain traced in source, OR `[agents: 2+]` demoted (not rejected) the same issue.
   - `[agents: 2+]` does NOT override a concrete refutation — demote to LEAD if refutation is uncertain.
   - No deployer-intent reasoning — evaluate what the code _allows_, not how the deployer _might_ use it.

4. **Fix verification** (confidence ≥ 80 only): trace the attack with fix applied; verify no new DoS, panic, or broken invariants; list all locations if the pattern repeats. If no safe fix exists, omit it with a note.

5. **Format and print** per `report-formatting.md`. Exclude rejected items. If `--file-output`: also write to file.

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
