# Contributing

## Pull requests

Pull request titles must follow
[Conventional Commits](https://www.conventionalcommits.org/):

```text
<type>[optional scope][!]: <description>
```

Allowed types are `feat`, `fix`, `docs`, `test`, `ci`, `refactor`, `perf`,
`chore`, and `revert`.

Examples:

```text
feat(branch): add merged-branch filtering
fix(config): expand configured home directories
docs: clarify shell completion installation
feat(cli)!: rename the repos command
```

Pull requests must also include a short description. The `PR policy` workflow
checks both requirements.

## Local checks

Run the project checks before opening a pull request:

```sh
just check-all
```
