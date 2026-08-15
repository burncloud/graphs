#!/usr/bin/env bash
set -euo pipefail

prompt="$(cat)"
if [[ "$prompt" != *"PAGE: overview"* ]]; then
  echo "patch-agent only supports the overview smoke run" >&2
  exit 64
fi

: "${BURNCLOUD_GRAPHS_PATCH_URL:?BURNCLOUD_GRAPHS_PATCH_URL is required}"
patch_file="$(mktemp)"
trap 'rm -f "$patch_file"' EXIT

curl -fsSL "$BURNCLOUD_GRAPHS_PATCH_URL" -o "$patch_file"
git apply --check "$patch_file"
git apply "$patch_file"

# The graph's build gate uses rustfmt --check. The implementation agent is
# allowed to format its own changes before handing control back to the graph.
cargo fmt --all
