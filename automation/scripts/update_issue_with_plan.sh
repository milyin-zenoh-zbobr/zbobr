#!/usr/bin/env bash
# Update an issue with implementation plan and set milestone to PENDING
# Must be run from domain project directory (with .zbobr.env)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"

usage() {
  cat <<USAGE
Usage: $(basename "$0") <issue_number> <plan_file>
Example: $(basename "$0") 1 /path/to/plan.md

Must be run from domain project directory (with .zbobr.env)

This script appends a plan file to an existing issue body under a separator
and sets the issue milestone to PENDING.

Requirements: gh CLI (GitHub CLI) and an authenticated session (gh auth login).
USAGE
}

if [[ "${1:-}" == "-h" ]] || [[ "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ -z "${1:-}" ]] || [[ -z "${2:-}" ]]; then
  usage
  exit 1
fi

ISSUE="$1"
PLAN_FILE="$2"

if ! command -v gh >/dev/null 2>&1; then
  echo "gh CLI not found, please install it: https://cli.github.com/" >&2
  exit 2
fi

if ! gh auth status >/dev/null 2>&1; then
  echo "gh CLI not authenticated; run 'gh auth login' and try again." >&2
  exit 3
fi

if [[ ! -f "$PLAN_FILE" ]]; then
  echo "Plan file not found: $PLAN_FILE" >&2
  exit 4
fi

echo "Updating issue #$ISSUE in $ZBOBR_DOMAIN_REPO..."

TMP_BODY="$(mktemp /tmp/issue_body.XXXXXX)"
trap 'rm -f "$TMP_BODY"' EXIT

orig_body=$(gh api repos/"$ZBOBR_DOMAIN_REPO"/issues/"$ISSUE" --jq .body)
printf "%s\n\n---\n\nImplementation plan:\n\n" "$orig_body" > "$TMP_BODY"
cat "$PLAN_FILE" >> "$TMP_BODY"

# Ensure 'PENDING' milestone exists
gh milestone create "PENDING" --repo "$ZBOBR_DOMAIN_REPO" >/dev/null 2>&1 || true
milestone_num=$(gh api repos/"$ZBOBR_DOMAIN_REPO"/milestones --jq '.[] | select(.title=="PENDING") | .number')

# Update issue body and milestone
gh api repos/"$ZBOBR_DOMAIN_REPO"/issues/"$ISSUE" -X PATCH -f body="$(cat "$TMP_BODY")" -f milestone="$milestone_num"

echo "Updated issue #$ISSUE in $ZBOBR_DOMAIN_REPO (milestone PENDING)."
