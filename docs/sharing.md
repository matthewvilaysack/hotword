# Sending a workflow to someone

## The one constraint everything follows from

A workflow is shell that runs automatically, before the model answers, fired by a phrase the person would have typed anyway.

So sending someone a workflow is sending them code that executes on their machine without them running anything.
That is not a caveat at the end of this document, it is the whole design.
Every choice below exists because the naive version of this feature, a link that installs a workflow, is a malware delivery mechanism aimed at exactly the population that runs coding agents with broad permissions.

The mechanism is already almost entirely built.
`hotword show` prints a workflow's TOML and `hotword save` validates one on stdin and writes it.
What is missing is not plumbing, it is the review gate and the provenance to review against.

## Sending

```
hotword share <name>
```

Prints the workflow's TOML with a short header, ready to paste into Slack, a message, or an email.

Plain text rather than a link, on purpose.
A teammate should see the steps in the message itself and be able to judge them before anything touches their machine.
A URL hides exactly the thing they need to look at, and a URL can change after they read it.

`--gist` creates a secret gist and prints the link, for a workflow too long to paste.
It is the second-best option and is offered as such.

## Receiving

```
hotword add --from <path | url | - >
```

The gate, in order.

Parse and validate first, so a malformed file fails before anything is shown as if it were real.

Print every step in full: its name, its timeout, and the exact command line.
Not a summary, not a count.
The command is the thing being consented to.

Flag the steps that deserve a second look, by simple inspection rather than by pretending to understand shell.
Anything that reaches the network, writes outside the working directory, pipes to a shell, invokes a package manager, or reads a path that looks like a credential.
The flag is a prompt to read, not a verdict.

Then ask, once, with the default on no.
Refuse entirely when not attached to a terminal unless `--yes` is passed, so a shared workflow can never land as a side effect of some other script.

## Provenance

The schema has no slot for where a workflow came from, and a shared workflow without one is anonymous the moment it is saved.

```toml
[meta]
from = "matthew"
source = "https://gist.github.com/..."
added = "2026-09-11"
digest = "sha256:..."
```

The digest is of the steps as reviewed.
`hotword list` marks a workflow whose steps no longer match what was consented to, which is the difference between a file someone edited and a file that changed under you.

Nothing here auto-updates.
A workflow you reviewed is the workflow that runs, and a newer version from the same source is a new review.

## Glide workflows

Glide's session-start injection is already a workflow in everything but name, so it ships as one:

```toml
name = "glide"
description = "Today's focus, so the agent knows what you are on"
on = ["session-start"]

[[steps]]
name = "priorities"
run = "glide prime"
```

The phrase half is new capability Glide has no route to today.
`triggers = ["what am I on", "what did I just do"]` running `glide focus show` is the cheap scoped read that avoids dumping the whole daily note into the context.

## Team workflows, and the honest limit

A team workflow shares a *workflow*, not *state*.

That distinction is the whole thing.
Sending a teammate a workflow that runs `glide focus show` shows them **their** focus, not yours, because Glide keeps a Markdown note on the person's own machine and there is no server.
Everyone running the same file against their own data is the shape that works with no backend, and it is genuinely useful: the same deploy check, the same PR review, the same repo status, so a report means the same thing in two people's sessions.

Sharing actual state, a board where everyone sees everyone's focus, needs a backend that does not exist and changes what Glide is.
That is a product decision rather than a feature, and it should not be smuggled in through a sharing feature.

## What this deliberately does not build

A registry or marketplace of community workflows.
Curating arbitrary auto-executing shell from strangers is a full-time safety problem, and a small list of examples in this repository carries the useful half at none of the cost.

Install straight from a URL with no review step, even behind a flag that sounds careful.

Auto-update of anything shared.

Any form of "trusted publisher" that skips the step listing.
The steps are short, reading them is the point, and a trust badge is how people stop reading them.
