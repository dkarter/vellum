#!/usr/bin/env bash
set -euo pipefail

cargo build --release --bin vlm

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary="$root/target/release/vlm"
slow_palette="$root/benchmarks/fixtures/slow-one-item.toml"
config="$root/benchmarks/fixtures/config/no-frecency"

hyperfine --shell=none --warmup 20 --runs 100 \
  --command-name "process floor" "$binary --version"

hyperfine --warmup 5 --runs 30 \
  --command-name "blocking slow source" \
  "XDG_CONFIG_HOME='$config' '$binary' '$slow_palette' --select-1" \
  --command-name "first frame and cancellation" \
  "'$root/scripts/benchmark-first-frame.exp' '$binary' '$slow_palette' '$config'"
