# Changelog

Every release gets a section here before it is tagged.
The release workflow refuses a tag whose version has no section.
Entries are written for the person upgrading: what changed for them, not which files moved.

## Unreleased

- `yardstick` example: a measured design critique of a page against named reference sites, fired by "critique my page", "yardstick", or "does this look generic".

## 0.1.0

First release.

- Workflows as TOML files with trigger phrases, optional session-start firing, and ordered shell steps.
- `hotword hook prompt` and `hotword hook session-start` for Claude Code and Codex, with an idempotent installer that repairs a moved binary path.
- Steps run in their own process group with per-step timeouts; missing binaries and directories skip with a reason.
- `HOTWORD_PROMPT` and `HOTWORD_TRIGGER` in every step's environment, so a step can read a PR number or a branch out of what was typed.
- Guided `hotword add` on a terminal, flags everywhere else.
- OpenCode plugin in `integrations/opencode`.
- Examples: `apple-status`, `repo-status`, and `pr-review`.
