# hotword: agent brief

A Rust CLI that runs shell workflows when a phrase appears in a prompt or a coding-agent session starts, and injects the report as hook context.
Zero runtime dependencies beyond a shell.

## Layout

- `src/workflow.rs`: the TOML schema, validation, trigger matching, and the store (a repo's `.hotword/` shadows `~/.config/hotword/`).
- `src/runner.rs`: runs steps through `sh -c` in their own process group, with per-step timeouts, and skips steps whose binary or `cwd` is missing.
- `src/report.rs`: text, JSON, and the hook envelope. The text form is what lands in the agent's context, so it is compact and capped.
- `src/hooks.rs`: edits Claude Code `settings.json` and Codex `hooks.json`, which share one shape. Idempotent; repairs a moved binary path.
- `src/cli.rs`: clap commands. Errors print as `error:` and `help:` lines on stdout, exit 1; usage errors exit 2.
- `src/interactive.rs`: the guided `add`, only reached on a terminal with no `--step`.
- `tests/`: one file per module, driving real commands in temp dirs. `tests/cli.rs` runs the built binary with `HOME` pointed at a sandbox.
- `examples/`: workflow files to copy.

## Rules

- Everything the CLI prints for an agent goes to stdout; diagnostics go to stderr. Hook subcommands never exit non-zero.
- Never add a runtime prompt outside `interactive.rs`, and never reach it unless stdin is a terminal.
- Keep `hooks.rs` shape-agnostic: it must not learn Claude-only or Codex-only fields.
- Tests first. Every behaviour in `src/` has a test that failed before the code existed.
- Commit subjects are plain imperative. No conventional-commit prefixes, no AI attribution trailers, no em dashes anywhere in prose or output.

## What an agent may do here

Read, build, test, and open a pull request on its own.
Editing a user's real `~/.claude/settings.json` or `~/.codex/hooks.json` is a `hotword install` a person runs; do not script it from a test or a hook.
