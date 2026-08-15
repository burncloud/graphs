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

# Deliberately do not run package- or workspace-wide formatters here. The graph
# must evaluate only the migration delta, while target-repository CI commands
# decide whether the resulting application is valid.
