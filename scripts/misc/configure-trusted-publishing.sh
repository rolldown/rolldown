#!/usr/bin/env bash
#
# Configure crates.io trusted publishing for every publishable crate in this
# workspace. Run once per crate to install the GitHub Actions OIDC trust
# relationship — after that, `release-crates.yml` can publish without a
# CARGO_REGISTRY_TOKEN.
#
# Requirements:
#   - CRATES_IO_TOKEN env var (a crates.io API token, `cargo login` token).
#   - The crate already exists on crates.io (you must publish the initial
#     version with a classic token first; trusted publishing only takes over
#     for subsequent releases).
#   - You are an owner of every crate to be configured.
#   - A GitHub Actions environment named `crates` exists in the repo (Settings
#     → Environments → New environment).
#
# Usage:
#   CRATES_IO_TOKEN=cio... ./scripts/misc/configure-trusted-publishing.sh

set -euo pipefail

if [[ -z "${CRATES_IO_TOKEN:-}" ]]; then
  echo "error: CRATES_IO_TOKEN env var is required" >&2
  exit 1
fi

REPO_OWNER="rolldown"
REPO_NAME="rolldown"
WORKFLOW_FILENAME="release-crates.yml"
ENVIRONMENT="crates"

CRATES_IO_URL="https://crates.io/api/v1/trusted_publishing/github_configs"

cd "$(dirname "$0")/../.."

crates=()
while IFS= read -r line; do
  crates+=("$line")
done < <(
  cargo metadata --no-deps --format-version=1 \
    | jq -r '.packages[] | select(.publish == null) | .name' \
    | sort
)

echo "Will configure trusted publishing for ${#crates[@]} crate(s):"
printf '  - %s\n' "${crates[@]}"
echo
echo "  Repository:    $REPO_OWNER/$REPO_NAME"
echo "  Workflow:      $WORKFLOW_FILENAME"
echo "  Environment:   $ENVIRONMENT"
echo

read -rp "Proceed? [y/N] " confirm
if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
  echo "Aborted."
  exit 0
fi

ok=0
failed=0
for crate in "${crates[@]}"; do
  body=$(jq --null-input \
    --arg crate "$crate" \
    --arg owner "$REPO_OWNER" \
    --arg name "$REPO_NAME" \
    --arg workflow "$WORKFLOW_FILENAME" \
    --arg env "$ENVIRONMENT" \
    '{github_config: {crate: $crate, repository_owner: $owner, repository_name: $name, workflow_filename: $workflow, environment: $env}}')

  response=$(curl --silent --show-error --write-out '\n%{http_code}' \
    -X POST "$CRATES_IO_URL" \
    -H "Authorization: $CRATES_IO_TOKEN" \
    -H "Content-Type: application/json" \
    -d "$body")
  status=$(printf '%s' "$response" | tail -n1)
  payload=$(printf '%s' "$response" | sed '$d')

  if [[ "$status" =~ ^2 ]]; then
    echo "  ✓ $crate"
    ok=$((ok + 1))
  else
    echo "  ✗ $crate [$status]: $payload"
    failed=$((failed + 1))
  fi
done

echo
echo "Done. $ok succeeded, $failed failed."
[[ $failed -eq 0 ]]
