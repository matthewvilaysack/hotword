# hotword: agent brief

Rust CLI. A phrase in a prompt, or a session start, runs shell steps and injects the report as hook context. No runtime dependencies beyond a shell.

## Layout

- `src/workflow.rs`: TOML schema, validation, matching, the store (`.hotword/` shadows `~/.config/hotword/`).
- `src/runner.rs`: runs steps through `sh -c` in their own process group with timeouts; skips missing tools and folders.
- `src/report.rs`: text, JSON, and the hook envelope, capped for the agent's context.
- `src/doctor.rs`: verdict, cause, and fixes per step, by string rules.
- `src/history.rs`: optional Dolt run history, one commit per run.
- `src/hooks.rs`: edits Claude Code `settings.json` and Codex `hooks.json`; idempotent, repairs a moved path.
- `src/ui/`: the terminal interface; `state.rs` is pure and tested, `view.rs` only draws.
- `src/cli.rs`: clap commands. Errors are `error:` and `help:` lines on stdout, exit 1; usage exits 2.
- `src/interactive.rs`: the guided `add`, only on a terminal with no `--step`.
- `tests/`: one file per module, real commands in temp dirs; `tests/cli.rs` drives the built binary.
- `examples/`: workflow files to copy. `site/`: the Vercel site; the banner and docs are generated at build.

## Rules

- Agent-facing output on stdout, diagnostics on stderr. Hook subcommands never exit non-zero.
- No prompts outside `interactive.rs`, and only when stdin is a terminal.
- `hooks.rs` stays shape-agnostic: no Claude-only or Codex-only fields.
- Tests first; every behaviour in `src/` had a failing test before its code.
- Commit subjects are plain imperative. No conventional-commit prefixes, no AI attribution, no em dashes.

## What an agent may do

Read, build, test, open a pull request.
Editing a real `~/.claude/settings.json` or `~/.codex/hooks.json` is a `hotword install` a person runs; never script it from a test or a hook.
