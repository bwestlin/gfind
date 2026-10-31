<!-- Allow this file to not have a first line heading -->
<!-- markdownlint-disable-file MD041 no-emphasis-as-heading -->

<!-- inline html -->
<!-- markdownlint-disable-file MD033 -->

<div align="center">

# `gfind`

**Find repositories, branches, commits, statuses, and stashes across collections
of Git repositories.**

[![Crates.io](https://img.shields.io/crates/v/gfind.svg)](https://crates.io/crates/gfind)
[![dependency status](https://deps.rs/repo/github/bwestlin/gfind/status.svg)](https://deps.rs/repo/github/bwestlin/gfind)
[![Build status](https://github.com/bwestlin/gfind/workflows/CI/badge.svg)](https://github.com/bwestlin/gfind/actions)

![Animated terminal demo of gfind](https://raw.githubusercontent.com/bwestlin/gfind/main/assets/gfind-demo.gif)

<sub>Mocked example output. No real repositories, branches, or stashes are shown.</sub>
</div>

`gfind` recursively discovers repositories below one or more search roots and
runs the same query across all of them. Output is concise, colorized in a
terminal, and uses paths relative to the current directory by default.

## Installation

Install the latest release from crates.io:

```sh
cargo install gfind
```

Or install the current source checkout:

```sh
cargo install --path .
```

## Quick start

```sh
# List every repository below the current directory.
gfind repos

# Show matching repositories in a directory tree.
gfind --tree repos

# Search repository paths and remote URLs.
gfind repos api

# Find local branches across repositories.
gfind branch main

# Find dirty worktrees or branches ahead of their upstream.
gfind status

# Search stashes by reference or subject.
gfind stash login
```

Pass one or more explicit search roots with `--path`:

```sh
gfind --path ~/src --path ~/work branch feature/login
```

## Commands

| Command | Alias | Description |
| --- | --- | --- |
| `repos [QUERY]` | `r` | List repositories or match their paths and remotes |
| `branch QUERY` | `b` | Find local, remote, or current branches |
| `commit REV` | `c` | Find repositories containing a commit |
| `status` | `s` | Find dirty worktrees or unpushed commits |
| `stash [QUERY]` | `sh` | List, search, or print stashes |
| `config init` | `cfg` | Create a documented configuration file |
| `completions SHELL` | `comp` | Generate shell completions |

Run `gfind <command> --help` for all command-specific options.

### Repository queries

Repository queries match both paths and remote URLs:

```sh
gfind repos gfind
gfind repos '^git_.*/gfind$' --regex
```

When only a remote URL matches, `gfind` prints that remote below the repository.

### Branch queries

Branches are searched locally by default:

```sh
gfind branch main
gfind branch feature/search --remote
gfind branch feature/search --all
gfind branch main --current
```

Local branch results include their commit and upstream freshness by default:

```text
project
  local: main
    commit: 2a46717f4c4ad7509291df3f9ec704f07671fa5c
    status: up-to-date with origin/main @ 2a46717f4c4ad7509291df3f9ec704f07671fa5c
    current: yes
```

Use `--short-commit-hash` or `--no-commit-hash` to change hash output. Use
`--no-upstream-status` to omit freshness. `--origin-status` checks the live
`origin` branch first and falls back to the local upstream tracking ref.

### Commit, status, and stash queries

```sh
# Find a commit object, optionally requiring reachability from HEAD.
gfind commit abc1234
gfind commit abc1234 --reachable

# Limit status results.
gfind status --dirty-only
gfind status --unpushed-only

# Search or inspect stashes.
gfind stash
gfind stash login
gfind stash --stash 0 --print
gfind stash login --patch
```

## Matching

Repository, branch, and stash queries support five matching modes:

| Mode | Flag | Behavior |
| --- | --- | --- |
| Contains | `--contains` | Case-insensitive substring matching (default) |
| Matches | `--query-mode matches` | Alias for contains |
| Exact | `--exact` | Case-insensitive full-string matching |
| Fuzzy | `--fuzzy` | Ordered character matching |
| Regex | `--regex` | Regular-expression matching |

## Search roots

The current working directory is searched by default. Configure persistent
roots or repeat `--path` on the command line:

```sh
gfind --path ~/src --path ~/work repos
```

Repository paths are relative to the current directory unless `--absolute` is
set. Directories named `target` are excluded by default. Replace that exclusion
with one or more regular expressions, or disable it for one command:

```sh
gfind --exclude-dir '^node_modules$' --exclude-dir '^vendor/generated$' repos
gfind --no-exclude-dirs repos
```

Use `--verbose` to print traversal and Git diagnostics to standard error.
Set `NO_COLOR=1` to disable ANSI color.

## Tree output

Use `--tree` with any search command to group matching repositories by directory:

```sh
gfind --tree repos
gfind --path ~/src --tree branch main --short-commit-hash
gfind status --tree
gfind commit HEAD --tree
gfind stash login --patch --tree
```

For example, `gfind --tree repos` could print:

```text
. — 3 repos
├── apps — 1 repo
│   └── cli
└── work — 2 repos
    ├── api
    └── web
```

Intermediate directories show the number of matching repositories in their
subtree. Only matching repositories and their ancestors appear; overlapping
search roots do not duplicate repositories. Nested repositories retain their
own results. Branches, commits, status reasons, remote matches, and stash
contents keep their usual details beneath or beside each repository.

Trees use paths relative to the current directory by default, with `.` as the
root and `..` for results outside it. `--absolute` starts the hierarchy at the
filesystem root. Directory siblings are sorted by name. Color and query
highlighting follow the usual terminal settings; pipes receive plain text.
Tree results are buffered until the search completes.

Flat output remains the default. Set `tree = true` in the configuration to
prefer trees, and use `--no-tree` to request flat output for one command.

## Configuration

Create `~/.gfind/config.toml` with documented defaults:

```sh
gfind config init
```

Existing files are preserved unless `--force` is passed. Use `--config` to
select another location.

```toml
paths = ["~/src", "~/work"]
include_cwd = false
absolute_paths = false
tree = false
exclude_dirs = ["^target$"]
branch_search = "local"
repo_query = "contains"
branch_query = "contains"
stash_query = "contains"
branch_hash = "full"
branch_upstream_status = true
branch_origin_status = false
```

Command-line options override configuration. Query modes may be `contains`,
`matches`, `exact`, `fuzzy`, or `regex`; branch search may be `current`, `local`,
`remote`, or `all`; and branch hashes may be `full`, `short`, or `none`.

Help output displays effective defaults and any configuration overrides as
equivalent command-line flags:

```sh
gfind --help
gfind branch --help
gfind --config ./gfind.toml branch --help
```

## Shell completions

Generate completions for Bash, Elvish, Fish, PowerShell, or Zsh:

```sh
gfind completions bash
gfind completions fish
gfind completions zsh
```

Install the generated script according to your shell's conventions. For
example, load Bash completions for the current session with:

```sh
source <(gfind completions bash)
```

## Development

The repository uses [just](https://github.com/casey/just) as its task runner.
Run `just` to list available recipes, or run all local checks with:

```sh
just check-all
```

Regenerate the mocked terminal demo after changing console output:

```sh
scripts/render-demo.sh
```

The script requires ImageMagick and does not inspect any real repositories.

Releases use `cargo-release` and `git-cliff`. See
[RELEASING.md](RELEASING.md) for the complete local release procedure.
See [CONTRIBUTING.md](CONTRIBUTING.md) for pull request and commit conventions.

## License

`gfind` is available under the [MIT License](LICENSE).
