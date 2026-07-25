#!/usr/bin/env bash
set -euo pipefail

list_only=false
if [[ "${1:-}" == "--list" ]]; then
  list_only=true
  shift
fi

target="${1:-}"
if [[ -z "$target" ]]; then
  echo "Usage: scripts/test-spec.sh [--list] <scenario-id|capability|spec-path>" >&2
  exit 2
fi

if [[ "$target" =~ ^[A-Z][A-Z0-9]+-[0-9]{3}$ ]]; then
  ids=("$target")
else
  if [[ -f "$target" ]]; then
    spec_file="$target"
  else
    spec_file="openspec/specs/${target%/spec.md}/spec.md"
  fi
  if [[ ! -f "$spec_file" ]]; then
    echo "No OpenSpec file found for: $target" >&2
    exit 1
  fi
  ids=()
  while IFS= read -r id; do
    ids+=("$id")
  done < <(rg --no-filename --only-matching '\{#[A-Z][A-Z0-9]+-[0-9]{3}\}' "$spec_file" | tr -d '{#}')
fi

if [[ "${#ids[@]}" -eq 0 ]]; then
  echo "No scenario IDs found for: $target" >&2
  exit 1
fi

for id in "${ids[@]}"; do
  filter="$(tr '[:upper:]-' '[:lower:]_' <<<"$id" | tr -d '\n')"
  if [[ "$list_only" == true ]]; then
    printf '%s\n' "$filter"
  else
    cargo test "$filter" -- --nocapture
  fi
done
