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
