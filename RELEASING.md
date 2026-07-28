# Releasing gfind

Releases are performed locally with
[`cargo-release`](https://github.com/crate-ci/cargo-release). Changelog entries
are generated from
[Conventional Commits](https://www.conventionalcommits.org/) by
[`git-cliff`](https://git-cliff.org/).

`git-cliff` owns changelog generation. `cargo-release` owns version updates,
verification, the release commit and tag, crates.io publication, and pushing to
GitHub.

## Prerequisites

Install the release tools:

```sh
cargo install --locked cargo-release git-cliff
```

Authenticate with crates.io before the first release:

```sh
cargo login
```

Release from an up-to-date, clean `main` branch. Ensure CI is green and that
the package version does not already exist on crates.io.

## Conventional Commits

Every commit after `v0.1.0` must use this form:

```text
<type>[optional scope][!]: <description>
```

Examples:

```text
feat(branch): add merged-branch filtering
fix(config): expand the configured home directory
docs: clarify shell completion installation
feat(cli)!: rename the repos command
```

The changelog includes `feat`, `fix`, `perf`, `refactor`, `docs`, and `revert`
commits. Maintenance-only `build`, `ci`, `chore`, `style`, and `test` commits
are intentionally omitted. A `!` before the colon or a `BREAKING CHANGE:`
footer marks a breaking change.

Validate all commits since the latest release tag:

```sh
just changelog-check
```

Preview the corresponding changelog section:

```sh
just changelog-preview
```

The commits before `v0.1.0` predate this policy. Their handling is pinned by
commit SHA in `cliff.toml`, solely to bootstrap the first changelog. Any other
unclassified commit fails generation.

## Choose a version

Choose the bump from the user-visible changes since the previous release:

- `patch` for backwards-compatible fixes.
- `minor` for backwards-compatible features.
- `major` for breaking changes.

While the project is below `1.0.0`, use an explicit version if the intended
pre-1.0 compatibility policy does not map cleanly to those levels:

```sh
just release-dry-run 0.2.0
```

For the first release, use the existing manifest version without incrementing
it:

```sh
just release-dry-run release
```

## Preview the release

Run the normal project checks:

```sh
just fmt-check
just check
just lint
just test
```

Then run cargo-release in its default dry-run mode:

```sh
just release-dry-run patch
```

Replace `patch` with `minor`, `major`, `release`, or an explicit version.
During a dry run the generated changelog is printed but `CHANGELOG.md` is not
modified.

Review the proposed version, packaged files, changelog, release commit, tag,
publish operation, and push before continuing.

## Execute the release

The following command is intentionally mutating and publishes permanently:

```sh
just release patch
```

`cargo-release` will:

1. Verify the branch, remote state, and clean worktree.
2. Update the version in `Cargo.toml` and `Cargo.lock`.
3. Prepend the generated release section to `CHANGELOG.md`.
4. Verify the crates.io package.
5. Create `chore(release): prepare vX.Y.Z`.
6. Publish the crate to crates.io.
7. Create the annotated `vX.Y.Z` tag.
8. Push the release commit and tag to `origin`.

For the first `0.1.0` release, execute:

```sh
just release release
```

Do not interrupt the command once publication begins.

## Verify

Confirm the release after the command completes:

```sh
git status --short
git show vX.Y.Z
cargo search gfind --limit 1
cargo install gfind --version X.Y.Z
```

Also verify the tag on GitHub and the package page on crates.io.

## If something fails

Before crates.io publication, inspect the worktree and either fix the problem
or restore the release changes manually. Do not delete or reset release work
without first checking which cargo-release steps completed.

After crates.io publication, never reuse the same version. Published crate
versions are immutable. If a release is broken, yank it and publish a corrected
version:

```sh
cargo yank --version X.Y.Z
```
