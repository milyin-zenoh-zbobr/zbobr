#!/bin/bash
# Cleanup workspace by removing directories for closed issues
# Must be run from domain project directory (with .zbobr.env)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"

DRY_RUN=false

print_usage() {
  echo "Usage: $0 [--dry-run]"
  echo ""
  echo "Scans workspace and removes directories for closed issues."
  echo "Must be run from domain project directory (with .zbobr.env)"
  echo ""
  echo "Options:"
  echo "  --dry-run    Show what would be deleted without actually deleting"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)
      DRY_RUN=true
      shift
      ;;
    -h|--help)
      print_usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1"
      print_usage
      exit 1
      ;;
  esac
done

if [[ ! -d "$ZBOBR_WORKSPACE" ]]; then
  echo "Workspace directory does not exist: $ZBOBR_WORKSPACE"
  exit 0
fi

echo "Scanning workspace: $ZBOBR_WORKSPACE"
if [[ "$DRY_RUN" == "true" ]]; then
  echo "DRY RUN - no files will be deleted"
fi
echo ""

# Find all issue directories
for dir in "$ZBOBR_WORKSPACE"/issue#*; do
  [[ -d "$dir" ]] || continue

  # Extract issue number from directory name
  dirname="$(basename "$dir")"
  issue_number="${dirname#issue#}"

  # Check if issue is closed
  state=$(gh issue view "$issue_number" --repo "$ZBOBR_DOMAIN_REPO" --json state --jq '.state' 2>/dev/null || echo "UNKNOWN")

  if [[ "$state" == "CLOSED" ]]; then
    echo "Issue #$issue_number is closed - removing $dir"
    if [[ "$DRY_RUN" == "false" ]]; then
      rm -rf "$dir"
    fi
  else
    echo "Issue #$issue_number is $state - keeping $dir"
  fi
done

echo ""
echo "Cleanup complete."
