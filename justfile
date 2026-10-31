# Stick Smash tasks. `just` lists them.

default:
    @just --list

# Play
run *args:
    cargo run -p game -- {{args}}

# Play with dynamic linking for fast rebuilds
dev *args:
    cargo run -p game --features dev -- {{args}}

# All tests (sim, content, TS traces, buffer)
test *args:
    cargo test --workspace {{args}}

# Clippy with warnings as errors
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Format all crates
fmt:
    cargo fmt --all

# Everything CI should check
check: lint test

# Regenerate schemas and reformat content files after changing data types
bless:
    BLESS=1 cargo test -p sim --test content

# Save screenshots of a CPU match for visual checks
shots dir="/tmp/stick-smash-shots":
    cargo run -p game -- --screenshot {{dir}}

# Only the TS trace comparison (optionally filtered by fixture name)
traces filter="":
    TRACE_FILTER={{filter}} cargo test -p sim --test ts_trace -- --nocapture

# Re-record TS trace fixtures (needs ../stick-smash checkout)
record-traces:
    node tools/ts-trace/record.mjs

# One-shot TS → JSON content conversion (overwrites assets; needs ../stick-smash)
convert-ts:
    node tools/convert-ts/convert.mjs
    just bless

# Re-simulate every replay and verify checksums and completion
verify-replays dir:
    cargo run -p game -- --verify-replays {{dir}}
