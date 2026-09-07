# Changelog

Every release gets a section here before it is tagged.
The release workflow refuses a tag whose version has no section.
Entries are written for the person upgrading: what changed for them, not which files moved.

## Unreleased

## 0.2.0

- `hotword ui`: a lazygit-shaped terminal interface with live step-by-step runs, prompt runs, filtering, and in-place editing, in the City-783 palette.
- `hotword doctor`: runs a workflow (or one step) and gives every step a verdict, a cause, and fixes; `d` in the terminal interface shows the same.
- `hotword history`: optional run history in a Dolt repository, one commit per run, with `history changes <workflow>` for what moved since the previous run.
- `hotword save` reads a workflow on stdin, validates, and writes it; `hotword list --json` for other front ends.
- Steps see `HOTWORD_PROMPT` and `HOTWORD_TRIGGER`; `hotword run --prompt` supplies them by hand.
- `pr-review` example: the PR, its description, checks, files, the diff without lockfiles, and the comment history filtered to unresolved threads, human conversation, and verdicts.

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
