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
