# hotword

![hotword: say the phrase, the shell runs first](https://hotword-dusky.vercel.app/banner.svg)

> **Say the phrase. The shell runs first.**
> A phrase you would type anyway fires a workflow of shell steps, and the report lands in your coding agent's context before it answers.

hotword turns "check the deploy" or "review pr 42" into deterministic commands that run before the model sees your prompt.
No guessing which commands exist, no five tool calls to rediscover the state of a repo, and no tokens spent on it.

| Layer | What it is | Where it lives |
| :--- | :--- | :--- |
| **1. Phrase** | A trigger that appears in the prompt, or a session-start event | `triggers = [...]`, `on = ["session-start"]` |
| **2. Steps** | Shell commands run in order, each on its own timeout, skipped with a reason when a tool is missing | `[[steps]]` in one TOML file |
| **3. Context** | A compact report handed to Claude Code, Codex, or OpenCode through their hook contract | `hotword hook prompt`, `hotword hook session-start` |

**Start here:** [Site](https://hotword-dusky.vercel.app) | [Docs](https://hotword-dusky.vercel.app/docs) | [Design](https://hotword-dusky.vercel.app/docs/design) | [Releasing](https://hotword-dusky.vercel.app/docs/releasing) | [Changelog](https://hotword-dusky.vercel.app/docs/changelog) | [Mac app](https://hotword-dusky.vercel.app/#mac)

## Quick start

```sh
curl -fsSL https://hotword-dusky.vercel.app/install | sh
hotword install
```

The installer picks the release for your machine, checks it against the checksums that release published, and puts `hotword` in `~/.local/bin` without sudo.
`hotword install` registers the hooks in Claude Code's settings; `--agent codex` does the same for Codex.

### Every way to install it

<details>
<summary><b>curl</b>, the one-line installer (shown above)</summary>

```sh
curl -fsSL https://hotword-dusky.vercel.app/install | sh
curl -fsSL https://hotword-dusky.vercel.app/install | sh -s -- --version 0.1.0 --prefix ~/bin
```

Needs only curl and tar. Pin a version with `--version`, choose a directory with `--prefix`, or `--dry-run` to see what it would do.
</details>

<details>
<summary><b>Homebrew</b>, macOS and Linux</summary>

```sh
brew install matthewvilaysack/tap/hotword
brew upgrade hotword
```

The formula installs the same release tarball the installer uses, checksums included, and puts the example workflows under `$(brew --prefix)/share/hotword/examples`.
</details>

<details>
<summary><b>Release tarball</b>, by hand</summary>

Every release on the [releases page](https://github.com/matthewvilaysack/hotword/releases) ships macOS and Linux builds for both architectures with a `SHA256SUMS`.
Verify, untar, and copy `hotword` somewhere on your PATH.
</details>

<details>
<summary><b>cargo</b>, from source</summary>

```sh
cargo install --git https://github.com/matthewvilaysack/hotword
```

Or in a checkout, `make install` builds it, registers the Claude Code hooks, and seeds the `deploy-status` workflow if you have none.
</details>

Then:

```sh
hotword install            # registers the hooks in ~/.claude/settings.json
hotword install --agent codex
```

`install` is idempotent and repairs the binary path if it moves.
`hotword install --dry-run` shows the file it would touch.
`hotword uninstall` takes the hooks back out and leaves everything else in the file alone.

Codex asks you to trust new hooks: open `/hooks` in Codex after installing.

OpenCode uses JavaScript plugins rather than command hooks.
Copy `integrations/opencode/hotword.ts` to `~/.config/opencode/plugins/` (or a repo's `.opencode/plugins/`).
It appends a matching workflow's report to the user message and carries session-start reports in the system prompt.
The plugin typechecks against the published plugin types but has not been run inside OpenCode yet.

## Define a workflow

Guided, on a terminal:

```sh
hotword add deploy-status
```

Or in one line:

```sh
hotword add deploy-status \
  --trigger "check the deploy" \
  --step "branch: git status --short --branch" \
  --step "ci: gh run list --limit 5" \
  --step "unpushed: git log --branches --not --remotes --oneline"
```

That writes `~/.config/hotword/deploy-status.toml`:

```toml
name = "deploy-status"
triggers = ["check the deploy"]
timeout = 30
max_lines = 40

[[steps]]
name = "branch"
run = "git status --short --branch"

[[steps]]
name = "ci"
run = "gh run list --limit 5"

[[steps]]
name = "unpushed"
run = "git log --branches --not --remotes --oneline"
```

Add `on = ["session-start"]` to also run it when a session starts.
A step can set its own `timeout`, a `cwd` (with `~` expanded), and `requires = ["gh"]` when the first word of the command is not the binary it depends on.

### Share a workflow with your team

Pass `--project` to save into the current repo's `.hotword/` instead of your home config:

```sh
hotword add ci-status --project \
  --trigger "check ci" \
  --step "workflow: gh run list --branch main --workflow 'Build' --limit 3" \
  --step "head: git log -1 --format='%h %s'"
```

That writes `.hotword/ci-status.toml` inside the repo, not `~/.config/hotword/`.
Commit it, and every teammate's agent learns the "check ci" phrase on their next `git pull`; nobody writes a hook by hand.
A project workflow shadows a user workflow of the same name, so a repo can override a personal default without either side editing the other's file.

Steps see the text that fired them as `HOTWORD_PROMPT` and the matched phrase as `HOTWORD_TRIGGER`, so "review pr 42" can hand 42 to a step.
`hotword run <name> --prompt "..."` supplies the same thing by hand.

The `examples/` directory has four to start from:

- `repo-status`, a generic session-start brief: branch, unpushed commits, worktrees.
- `pr-review`, a review session in one phrase: the PR, its description, checks, the diff, and the comment history filtered to unresolved review threads, human conversation with bots dropped, and the review verdicts. Say "review pr" on a checked-out branch or "review pr 42".
- `deploy-status`, the checkout, CI runs, and open pull requests in one phrase, with steps that skip cleanly on a machine without `gh`.
- `yardstick`, a design critique in one phrase: "critique my page http://localhost:3000 vs stripe.com,linear.app" runs [yardstick](https://github.com/matthewvilaysack/yardstick) and hands the agent the side-by-side numbers, the key moves, and a recommended direction before it answers. With no references named it uses your first saved theme.

## Run and inspect

```sh
hotword                        # list workflows, where they live, what fires them
hotword ui                     # the terminal interface, see below
hotword run deploy-status       # run one now
hotword run deploy-status --json
hotword run deploy-status --full
hotword run deploy-status --strict   # exit 1 if any step failed, for CI
hotword run pr-review --prompt "review pr 42"
hotword match "hey check the deploy"   # which workflows would fire
hotword show deploy-status
hotword edit deploy-status      # opens $EDITOR, re-validates on save
hotword remove deploy-status
hotword list --json            # the listing for other front ends
hotword save < workflow.toml   # validate and write a workflow from stdin
```

### Debug a broken workflow

```sh
hotword doctor deploy-status
hotword doctor deploy-status --step open-prs   # rerun one step with full output
```

`doctor` runs the workflow, then gives every step a verdict with the cause in plain words and the fixes to try: a tool missing from PATH, a `cwd` that does not exist here, a command that wants `gh auth login`, a host it could not reach, a grep that exited 1 because it found nothing, a step that hit its timeout.
It names the step to start with; fix that one, rerun just that step, move to the next.
The same diagnosis is behind `d` in `hotword ui` and the Debug button in the Mac app.

### The terminal interface

`hotword ui` is a lazygit-shaped view: workflows and their steps on the left, the report on the right, keys along the bottom.
`enter` runs, `p` runs with a prompt, `d` explains the last run's failures, `/` filters, `e` opens the file in `$EDITOR`, `?` lists everything.
Runs stream in step by step.

### Run history with Dolt

```sh
hotword history init           # once; needs dolt on PATH
hotword history                # the last 20 runs, any workflow
hotword history changes repo-status   # steps that changed since the previous run
```

Once initialised, every `hotword run` and every hook run becomes a commit in a [Dolt](https://github.com/dolthub/dolt) repository under `~/.config/hotword/history`: a `runs` table, a `steps` table, and a `latest` table per workflow.
`changes` uses Dolt's own diff of `latest`, so "what moved since yesterday's status check" is one command.
`dolt log`, `dolt diff`, and `dolt sql` work in that directory, and `dolt remote add` plus `dolt push` share the whole record with a team.

A run report looks like this:

```
hotword: deploy-status
steps[3]{name,status,exit,ms}:
  branch,ok,0,18
  ci,fail,1,340
  unpushed,skip,-,0

branch:
  ## main...origin/main

ci (exit 1):
  ...

unpushed: skipped, git not on PATH
```

Steps run in order and all of them run even after a failure.
A step whose binary is not on PATH, or whose `cwd` does not exist, is skipped with the reason, so a shared workflow still works on a machine that has only some of the tools.
A step that passes its timeout is killed along with anything it started.

## How the hook works

`hotword hook prompt` reads the event JSON the agent sends on stdin, runs every workflow whose trigger phrase appears in the prompt, and prints:

```json
{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"hotword: deploy-status\n..."}}
```

`hotword hook session-start` does the same for workflows with `on = ["session-start"]`.
Both read the `cwd` field from the payload to find the repo's `.hotword/`, stay under the 10,000 character context cap the agents enforce, and never block the prompt: any problem goes to stderr and the exit code stays 0.

## Where this started

It began as hook hacking.
Claude Code exposes a small lifecycle, documented in the [hooks reference](https://code.claude.com/docs/en/hooks): a shell command gets JSON on stdin when a session starts or a prompt is submitted, and whatever it prints as `additionalContext` lands in the model's context before the next request.
The first version was a bash script matched on one word that opened a dashboard.
The interesting part turned out to be the shape underneath: a phrase is a cheap, honest trigger, and a hook that runs deterministic commands puts facts in front of the agent instead of making it guess.
hotword is that pattern made reusable, with the hook contract handled once so a workflow is only the phrase and the steps.

Reference: [Claude Code hooks](https://code.claude.com/docs/en/hooks). Codex uses the same shape; its guide is at [learn.chatgpt.com/docs/hooks](https://learn.chatgpt.com/docs/hooks).

## Develop

```sh
make test            # 68 tests, runs real shell commands in temp dirs
make lint            # clippy with warnings as errors
make format-check
```

Zero runtime configuration beyond the TOML files.
`HOTWORD_HOME` overrides the user workflow directory, which is how the tests stay isolated.
Releases are annotated tags; `docs/releasing.md` has the four steps and what the workflow enforces.
The site at [hotword-dusky.vercel.app](https://hotword-dusky.vercel.app) deploys from `site/` on every push to main through Vercel.
The banner is generated at build time: `bun scripts/build-banner.mjs` turns the wordmark and taglines into paths, writes `site/banner.svg` with the CSS tear, and rasterizes one torn frame to `site/banner.png` for social previews. `make banner` does the same locally.
