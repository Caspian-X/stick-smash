# Stick Smash tasks. `just` lists them.

default:
    @just --list

# Play
run *args:
    cargo run -p game -- {{args}}

# Play an optimized build (judge game feel with this one)
release *args:
    cargo run --release -p game -- {{args}}

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

# Fail on unformatted code
fmt-check:
    cargo fmt --all -- --check

# Everything CI should check
check: fmt-check lint test

# Regenerate schemas and reformat content files after changing data types
bless:
    BLESS=1 cargo test -p sim --test content -- --test-threads=1

# Save all screenshot sections, or a comma-separated subset
shots dir="/tmp/stick-smash-shots" sections="":
    cargo run -p game -- --screenshot "{{dir}}" --shots "{{sections}}"

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

# Detailed seat-swapped CPU matchups: damage, tips, whiffs, KO moves and recovery deaths
balance seeds="16" fighter="":
    BALANCE_SEEDS={{seeds}} BALANCE_FIGHTER="{{fighter}}" cargo test --release -p sim --test stages fighter_balance_diagnostics -- --ignored --nocapture

# Regenerate the README screenshots in docs/images (needs ImageMagick 7 with WebP)
readme-shots:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v magick >/dev/null || { echo "readme-shots needs ImageMagick 7 (magick) with WebP support" >&2; exit 1; }
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    cargo run -p game -- --screenshot "$tmp" --shots select,results-replay,training,fighter-editor,stage-editor,stages
    mkdir -p docs/images
    for pair in 17a-match-citadel:match 05-select-ready:select 05c-stage-roulette:stage-roulette 14-training:training \
        12-results:results 03b-editor-move:fighter-editor 03q-stage-moving-sandbox:stage-editor; do
        magick "$tmp/${pair%%:*}.png" -resize 1280x -quality 90 -define webp:method=6 "docs/images/${pair#*:}.webp"
    done
    du -ch docs/images/*.webp
