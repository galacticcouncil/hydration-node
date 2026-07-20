# AGENTS.md - hydration-node

This is the Codex entrypoint for the repository.

Follow the project guidance in `CLAUDE.md`; it is the shared repo instruction
file for build, test, style, PR, and domain conventions.

## Shared AI skills

Repo-local AI skills live in `ai_skills/` so they can be used by multiple coding
agents.

When the user invokes a skill by name, or the task clearly matches a skill
description, read the corresponding `ai_skills/<skill-name>/SKILL.md` file and
follow its instructions. Resolve any relative paths in a skill from that skill's
directory.

If a skill references tool names from another agent environment, use the closest
available Codex equivalent:
- `Read` -> file reads such as `sed`, `rg`, or editor context
- `Glob` -> `rg --files` or `find`
- `Grep` -> `rg`
- `Bash` -> shell commands
- `WebFetch` -> web access when available and permitted
- `Agent` -> Codex sub-agents only when the user has explicitly asked for
  delegated or parallel agent work

Available shared skills:
- `hydration_cl0wdit` - security audit workflow for Substrate runtime and pallet
  code.

## runtime interaction graph

For runtime dependency and interaction questions, use the bounded AI-facing
query interface documented in `scripts/runtime-interaction-graph/README.md`.
Prefer `query_graph.py` over loading `interaction-graph.json` or the interactive
HTML into model context. Use the generated `graph-scale.svg` for a compact visual
overview of graph size, activity, domains, edge kinds, and evidence provenance.

## User working preferences

- Do not run `git commit` or stage changes unless the user explicitly authorizes
  that specific commit. Leave completed changes unstaged by default.
- When authorized to commit, stay on the current branch unless the user asks for
  a new branch.
- Use a single short commit subject with no body or trailers. Do not add AI
  attribution. Follow wording supplied by the user; otherwise use the repository
  convention where applicable.
- Prefer lowercase prose in responses, summaries, headings, and proposed commit
  messages. Preserve the canonical capitalization of code identifiers and names.
- Default to no new explanatory comments. Add comments only for non-obvious
  runtime invariants or constraints, never to narrate a diff.
- Run `cargo fmt` after Rust changes.
- Prefer Makefile targets for builds and tests. For direct cargo commands, export
  the Makefile's required `RUSTFLAGS` and `CXXFLAGS` separately before running
  cargo; do not use inline environment prefixes.
- After version changes, run an appropriate cargo check so `Cargo.lock` remains
  synchronized. Version every changed crate: use a minor bump for source changes
  and a patch bump for test-only changes, and keep the runtime crate version and
  `spec_version` aligned.
- For comparison tests, use the reference implementation in its original
  language and toolchain instead of translating it.
- When the user needs to copy generated data such as an encoded call or a long
  hex value, write it to a suitable untracked project file so it is available in
  the IDE changes view.
