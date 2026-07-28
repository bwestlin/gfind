# gfind

Find things across Git repos

## Install locally

```sh
cargo install --path .
```

## Search roots

By default, `gfind` searches under the current working directory.

Repo paths are printed relative to the current working directory by default. Use
`--absolute` to print absolute paths, or configure `absolute_paths = true`.

You can pass one or more roots explicitly:

```sh
gfind --path ~/src --path ~/work repos
```

Directories named `target` are excluded from traversal by default. Supply one
or more replacement regexes with `--exclude-dir`, or disable exclusions for one
command with `--no-exclude-dirs`:

```sh
gfind --exclude-dir '^node_modules$' --exclude-dir '^vendor/generated$' repos
gfind --no-exclude-dirs repos
```

Use `--verbose` to print config, traversal, and git probe diagnostics to stderr:

```sh
gfind --verbose --path ~/src status
```

When stdout is a supported terminal, `gfind` colorizes output and shows matched
text on a dark gray background. Set `NO_COLOR=1` to disable ANSI color.

You can also configure default roots in `~/.gfind/config.toml`:

```sh
gfind config init
```

The config file is optional; when it is absent, `gfind` uses built-in defaults.

This writes a documented config file with commented examples. Existing files
are not overwritten unless `--force` is passed:

```sh
gfind config init --force
gfind --config ./gfind.toml config init
```

```toml
paths = ["~/src", "~/work"]
include_cwd = false
absolute_paths = false
exclude_dirs = ["^target$"]
branch_search = "local"
repo_query = "contains"
branch_query = "contains"
stash_query = "contains"
branch_hash = "full"
branch_upstream_status = true
branch_origin_status = false
```

`include_cwd` defaults to `true`. Use `--cwd` to include cwd even when
`include_cwd = false`, or `--no-cwd` to suppress cwd for one command.
`exclude_dirs` defaults to `["^target$"]`. Regexes match directory names and
paths relative to each search root. CLI exclusions replace configured values.
`branch_search` can be `current`, `local`, `remote`, or `all`.
`repo_query`, `branch_query`, and `stash_query` can be `contains`, `matches`,
`exact`, `fuzzy`, or `regex`; they default to `contains`.
`branch_hash` can be `full`, `short`, or `none`; it defaults to `full`.
`branch_upstream_status` defaults to `true`.
`branch_origin_status` defaults to `false`; when enabled, branch freshness first
checks the live `origin` remote branch and falls back to local upstream tracking
if the origin check is unavailable.

Help output prints effective built-in or configured defaults beside the options
through Clap. When the selected config file differs from the built-in defaults,
applicable changes are also listed as equivalent flags under
`Config overrides (equivalent CLI flags):` at the end:

```sh
gfind --help
gfind branch --help
gfind --config ./gfind.toml branch --help
```

## Commands

Top-level commands have visible aliases:

```text
repos: r
branch: b
commit: c
status: s
stash: sh
config: cfg
completions: comp
```

List all discovered repos:

```sh
gfind repos
```

Find repos by path or remote URL:

```sh
gfind repos gfind
gfind repos '^git_.*/gfind$' --regex
```

If a repo query matches only a remote URL, the matching remote line is printed
under the repo.

Find repos that have a local branch matching a query:

```sh
gfind branch main
gfind branch bwe/main-api --exact
gfind branch 'bwe/.+-ci' --regex
gfind branch mai --fuzzy
```

Matched branches are printed under each repo. If the matched branch is currently
checked out, that branch entry includes `current: yes`. Branch results include
full commit hashes and local upstream tracking freshness by default:

```text
project
  local: main
    commit: 2a46717f4c4ad7509291df3f9ec704f07671fa5c
    status: up-to-date with origin/main @ 2a46717f4c4ad7509291df3f9ec704f07671fa5c
    current: yes
```

The hash after `@` is the local upstream tracking ref hash by default. If
`--origin-status` or `branch_origin_status = true` is enabled, it is the live
remote `origin/<branch>` hash when that check succeeds.

Use short branch commit hashes:

```sh
gfind branch main --short-commit-hash
```

Suppress branch commit hashes:

```sh
gfind branch main --no-commit-hash
```

Force branch commit hashes when config disables them:

```sh
gfind branch main --commit-hash
```

`--commit-hash` is equivalent to `--full-commit-hash`.

Suppress branch upstream freshness:

```sh
gfind branch main --no-upstream-status
```

Force branch upstream freshness when config disables it:

```sh
gfind branch main --upstream-status
```

`--remote-status` and `--no-remote-status` are aliases for these upstream
status flags.

Use only local upstream tracking freshness when config enables live origin
checks:

```sh
gfind branch main --no-origin-status
```

Check the live `origin` branch before falling back to local upstream tracking:

```sh
gfind branch main --origin-status
```

`--remote-origin-status` and `--no-remote-origin-status` are aliases for these
origin status flags.

Only find repos where a branch is currently checked out:

```sh
gfind branch main --current
```

Find repos that have a remote branch:

```sh
gfind branch feature/search --remote
```

Find repos that have a local or remote branch, with matching refs shown:

```sh
gfind branch feature/search --all
```

Find repos that contain a commit object locally:

```sh
gfind commit abc1234
```

Commit matches print the queried revision under each matching repo.

Only match repos where the commit is reachable from `HEAD`:

```sh
gfind commit abc1234 --reachable
```

List repos with a dirty worktree or commits ahead of upstream:

```sh
gfind status
```

Limit status checks:

```sh
gfind status --dirty-only
gfind status --unpushed-only
```

List stashes:

```sh
gfind stash
```

Search stashes by ref or subject:

```sh
gfind stash login
gfind stash 'WIP.*auth' --regex
```

Print a specific stash across repos:

```sh
gfind stash --stash 0 --print
```

Print patches for matching stashes:

```sh
gfind stash login --patch
```

## Shell completions

Generate a completion script for Bash, Elvish, Fish, PowerShell, or Zsh:

```sh
gfind completions bash
gfind completions fish
gfind completions zsh
```

Source or install the generated script according to your shell's conventions.
For example, load Bash completions for the current session:

```sh
source <(gfind completions bash)
```
