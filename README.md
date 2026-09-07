# hotword

Type a phrase to your coding agent and a workflow of shell commands runs, with the results dropped into the agent's context before it answers.
Say "apple check status" and the agent already knows the CLI version, the auth state, the unpushed commits, and the open PRs, instead of spending five tool calls finding out.

The same workflows can run at session start, so every session opens with the state of the repo you are in.

Workflows are small TOML files.
A repo can commit its own under `.hotword/`, which is how a whole team ends up with the same "check status" behaviour in every agent session without anyone writing a hook by hand.

## Install

```sh
cargo install --path .
hotword install            # registers the hooks in ~/.claude/settings.json
hotword install --agent codex
```

`install` is idempotent and repairs the binary path if it moves.
`hotword install --dry-run` shows the file it would touch.
`hotword uninstall` takes the hooks back out and leaves everything else in the file alone.

Codex asks you to trust new hooks: open `/hooks` in Codex after installing.
OpenCode uses JavaScript plugins rather than command hooks, so there is no installer for it; a plugin that shells out to `hotword hook prompt` and feeds the `additionalContext` field back is all it needs.

## Define a workflow

Guided, on a terminal:

```sh
hotword add apple-status
```

Or in one line:

```sh
hotword add apple-status \
  --trigger "apple check status" \
  --step "version: maps-cli --version" \
  --step "plugins: maps-cli plugins doctor" \
  --step "unpushed: git log --branches --not --remotes --oneline"
```

That writes `~/.config/hotword/apple-status.toml`:

```toml
name = "apple-status"
triggers = ["apple check status"]
timeout = 30
max_lines = 40

[[steps]]
name = "version"
run = "maps-cli --version"

[[steps]]
name = "plugins"
run = "maps-cli plugins doctor"

[[steps]]
name = "unpushed"
run = "git log --branches --not --remotes --oneline"
```

Add `on = ["session-start"]` to also run it when a session starts.
A step can set its own `timeout`, a `cwd` (with `~` expanded), and `requires = ["gh"]` when the first word of the command is not the binary it depends on.

Pass `--project` to save into the current repo's `.hotword/` instead.
Project workflows shadow user workflows with the same name.

The `examples/` directory has two to start from: `apple-status` for the maps-cli team and `repo-status`, a generic session-start brief.

## Run and inspect

```sh
hotword                        # list workflows, where they live, what fires them
hotword run apple-status       # run one now
hotword run apple-status --json
hotword run apple-status --full
hotword run apple-status --strict   # exit 1 if any step failed, for CI
hotword match "hey apple check status"   # which workflows would fire
hotword show apple-status
hotword edit apple-status      # opens $EDITOR, re-validates on save
hotword remove apple-status
```

A run report looks like this:

```
hotword: apple-status
steps[3]{name,status,exit,ms}:
  version,ok,0,120
  plugins,fail,1,340
  unpushed,skip,-,0

version:
  maps-cli 0.2.68

plugins (exit 1):
  ...

unpushed: skipped, git not on PATH
```

Steps run in order and all of them run even after a failure.
A step whose binary is not on PATH, or whose `cwd` does not exist, is skipped with the reason, so a shared workflow still works on a machine that has only some of the tools.
A step that passes its timeout is killed along with anything it started.

## How the hook works

`hotword hook prompt` reads the event JSON the agent sends on stdin, runs every workflow whose trigger phrase appears in the prompt, and prints:

```json
{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"hotword: apple-status\n..."}}
```

`hotword hook session-start` does the same for workflows with `on = ["session-start"]`.
Both read the `cwd` field from the payload to find the repo's `.hotword/`, stay under the 10,000 character context cap the agents enforce, and never block the prompt: any problem goes to stderr and the exit code stays 0.

## Develop

```sh
make test            # 47 tests, runs real shell commands in temp dirs
make lint            # clippy with warnings as errors
make format-check
```

Zero runtime configuration beyond the TOML files.
`HOTWORD_HOME` overrides the user workflow directory, which is how the tests stay isolated.
