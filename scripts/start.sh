#!/usr/bin/env bash
set -euo pipefail
project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"
export PATH="$HOME/.cargo/bin:$HOME/.elan/bin:$PATH"
if [[ -x ./recurrence-lab ]]; then
  exec ./recurrence-lab "$@"
elif [[ -x ./target/release/recurrence-lab ]]; then
  exec ./target/release/recurrence-lab "$@"
else
  exec cargo run --locked -- "$@"
fi
