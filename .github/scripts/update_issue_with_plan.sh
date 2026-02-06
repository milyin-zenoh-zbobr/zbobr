#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $(basename "$0") <repo> <issue_number> <plan_file>
Example: $(basename "$0") milyin/copilot 1 /path/to/plan.md

This script appends a plan file to an existing issue body under a separator
and sets the issue milestone to PENDING.

Requirements: gh CLI (GitHub CLI) and an authenticated session (gh auth login).
USAGE
}

if [ "${1:-}" = "" ] || [ "${2:-}" = "" ] || [ "${3:-}" = "" ]; then
  usage
  exit 1
fi

REPO="$1"
ISSUE="$2"
PLAN_FILE="$3"

if ! command -v gh >/dev/null 2>&1; then
  echo "gh CLI not found, please install it: https://cli.github.com/" >&2
  exit 2
fi

if ! gh auth status >/dev/null 2>&1; then
  echo "gh CLI not authenticated; run 'gh auth login' and try again." >&2
  exit 3
fi

if [ ! -f "$PLAN_FILE" ]; then
  echo "Plan file not found: $PLAN_FILE" >&2
  exit 4
fi

TMP_BODY="$(mktemp /tmp/issue_body.XXXXXX)"
trap 'rm -f "$TMP_BODY"' EXIT

orig_body=$(gh api repos/"$REPO"/issues/"$ISSUE" --jq .body)
printf "%s\n\n---\n\nImplementation plan:\n\n" "$orig_body" > "$TMP_BODY"
cat "$PLAN_FILE" >> "$TMP_BODY"

# Ensure 'PENDING' milestone exists
gh milestone create "PENDING" --repo "$REPO" >/dev/null 2>&1 || true
milestone_num=$(gh api repos/"$REPO"/milestones --jq '.[] | select(.title=="PENDING") | .number')

# Update issue body and milestone
gh api repos/"$REPO"/issues/"$ISSUE" -X PATCH -f body="$(cat "$TMP_BODY")" -f milestone="$milestone_num"

echo "Updated issue $ISSUE in $REPO (milestone PENDING)."
