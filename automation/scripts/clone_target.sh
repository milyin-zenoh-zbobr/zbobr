#!/bin/bash
# Clone and fork a target repository for issue implementation
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
  echo "Must be run from domain project directory (with .zbobr.env)"
  exit 1
fi

# Validate required configuration
if [[ -z "$ZBOBR_FORK_OWNER" ]]; then
  echo "Error: ZBOBR_FORK_OWNER not set in .zbobr.env"
  exit 1
fi

REPO_NAME="${TARGET_REPO#*/}"
WORK_DIR="copilot/projects/$ZBOBR_DOMAIN_REPO_NAME"

echo "Configuration:"
echo "  Domain: $ZBOBR_DOMAIN_REPO"
echo "  ZBOBR_FORK_OWNER: $ZBOBR_FORK_OWNER"
echo "  ZBOBR_DEFAULT_MODEL: ${ZBOBR_DEFAULT_MODEL:-not set}"
echo ""

# Create work directory
mkdir -p "copilot/projects"

# Clone target repository if not already cloned
if [[ ! -d "$WORK_DIR" ]]; then
  echo "Cloning $TARGET_REPO..."
  gh repo clone "$TARGET_REPO" "$WORK_DIR"
fi

cd "$WORK_DIR"

# Create fork if it doesn't exist
FORK_REPO="$ZBOBR_FORK_OWNER/$ZBOBR_DOMAIN_REPO_NAME"
if ! gh repo view "$FORK_REPO" >/dev/null 2>&1; then
  echo "Creating fork to $ZBOBR_FORK_OWNER..."
  gh repo fork "$TARGET_REPO" --org "$ZBOBR_FORK_OWNER" --clone=false
  sleep 2  # Wait for fork to be ready
fi

# Add fork as remote
if ! git remote get-url fork >/dev/null 2>&1; then
  echo "Adding fork remote..."
  git remote add fork "https://github.com/$FORK_REPO.git"
fi

# Create feature branch
BRANCH_NAME="fix${ISSUE_NUMBER}/implementation"
if ! git rev-parse --verify "$BRANCH_NAME" >/dev/null 2>&1; then
  echo "Creating branch $BRANCH_NAME..."
  git checkout -b "$BRANCH_NAME"
else
  echo "Checking out existing branch $BRANCH_NAME..."
  git checkout "$BRANCH_NAME"
fi

echo ""
echo "Repository ready at: $WORK_DIR"
echo "Branch: $BRANCH_NAME"
echo "Fork: $FORK_REPO"
