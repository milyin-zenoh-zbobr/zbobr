#!/bin/bash
# Clone and fork a target repository for issue implementation
# This is a convenience wrapper around the clone_target function
# Must be run from domain project directory (with .zbobr.env)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"

TARGET_REPO="$1"
ISSUE_NUMBER="$2"

if [[ -z "$TARGET_REPO" ]] || [[ -z "$ISSUE_NUMBER" ]]; then
  echo "Usage: $0 <target_repo> <issue_number>"
  echo "Example: $0 zenoh/zenoh 123"
  echo ""
  echo "Clones target repo into issue workspace and configures fork."
  echo "Must be run from domain project directory (with .zbobr.env)"
  exit 1
fi

echo "Configuration:"
echo "  Domain: $ZBOBR_DOMAIN_REPO"
echo "  Fork owner: $ZBOBR_FORK_OWNER"
echo "  Workspace: $ZBOBR_WORKSPACE"
echo ""

WORK_DIR=$(clone_target "$TARGET_REPO" "$ISSUE_NUMBER")

echo ""
echo "Repository ready at: $WORK_DIR"
