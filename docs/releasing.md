# Releasing

A release is an annotated tag on `main`.
The tag is the whole ceremony: the workflow in `.github/workflows/release.yml` gates it, builds it, and publishes it.
Nobody uploads a binary by hand.

## Cut one

1. Move the `Unreleased` notes in `CHANGELOG.md` under a new `## X.Y.Z` heading, written for the person upgrading.
2. Set `version = "X.Y.Z"` in `Cargo.toml` and run `cargo build` so `Cargo.lock` follows.
3. Commit those two files together with the subject `Release X.Y.Z`, push, and wait for `ci` to go green.
4. Tag that commit and push the tag:

```sh
git tag -a vX.Y.Z -m "hotword X.Y.Z"
git push origin vX.Y.Z
```

A `-rc1` suffix makes a pre-release, the way Linux marks candidates.

## What the workflow enforces

- The tag names the same version as `Cargo.toml`.
- `CHANGELOG.md` has a section for that version.
- The tagged commit is on `main`.
- Format, clippy, and the tests pass on that exact commit before anything is built.

Then it builds `hotword` for macOS (Apple silicon and Intel) and Linux (x86_64 and arm64), tars each with the README, license, changelog, and examples, writes `SHA256SUMS`, and creates the GitHub release with the changelog section, the commit list since the previous tag, and the checksums as the notes.

## When it fails

The gate job's log says which rule tripped.
Fix on `main`, then delete and recreate the tag on the new commit:

```sh
git tag -d vX.Y.Z
git push origin :refs/tags/vX.Y.Z
```

A tag that never produced a release can be moved. A tag that did produce one is history; ship a patch instead.

## Where the shape comes from

Linux: annotated tags are the release, and release candidates are tags with an `-rc` suffix.
Cilium: a tag-triggered workflow publishes, and the notes are generated from the change list rather than typed into a form.
a2ui: every merge is gated by the same checks, so the release gate re-runs them rather than trusting the badge.
