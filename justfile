# Show available commands.
default:
    @printf "gfind task runner\n\n"
    @just --list

# Format Rust code in place.
fmt-write:
    cargo fmt

# Check Rust formatting.
fmt-check:
    cargo fmt --check

# Check that all Rust targets compile.
check:
    cargo check --locked --all-targets --all-features

# Run Clippy for tests and all features.
lint:
    cargo clippy --locked --tests --all-features -- -Dwarnings -Wclippy::pedantic -Aclippy::missing_errors_doc -Aclippy::must_use_candidate

# Run Rust tests with all features.
test:
    cargo test --locked --all-features

# Lint all Markdown files.
markdown-check:
    npx markdownlint-cli2 '**/*.md' '#target/**'

# Preview changes since the latest release tag.
changelog-preview:
    git-cliff --unreleased

# Validate unreleased commits and changelog generation.
changelog-check:
    git-cliff --unreleased > /dev/null

# Run all local checks.
check-all: fmt-check check lint test markdown-check changelog-check

# Preview a cargo-release run without changing the repository.
release-dry-run level:
    cargo release {{level}}

# Bump, publish, commit, tag, and push a release.
release level:
    cargo release {{level}} --execute

# Update the changelog from cargo-release without mutating it during dry runs.
_changelog-release version:
    @if [ "$DRY_RUN" = "true" ]; then git-cliff --unreleased --tag "v{{version}}"; else git-cliff --unreleased --tag "v{{version}}" --prepend CHANGELOG.md; fi
