#!/bin/bash
# Clone and fork a target repository for issue implementation
# Loads .zbobr.env from the domain project for configuration

set -e

DOMAIN_PROJECT="$1"
TARGET_REPO="$2"
ISSUE_NUMBER="$3"

if [[ -z "$DOMAIN_PROJECT" ]] || [[ -z "$TARGET_REPO" ]] || [[ -z "$ISSUE_NUMBER" ]]; then
  echo "Usage: $0 <domain_project> <target_repo> <issue_number>"
  echo "Example: $0 YoroolGui/copilot-zenoh zenoh/zenoh 123"
  exit 1
fi

REPO_NAME="${TARGET_REPO#*/}"
WORK_DIR="copilot/projects/$REPO_NAME"

# Load configuration from domain project's .zbobr.env
echo "Loading configuration from $DOMAIN_PROJECT..."
ENV_CONTENT=$(gh api "repos/$DOMAIN_PROJECT/contents/.zbobr.env" --jq '.content' 2>/dev/null | base64 -d 2>/dev/null || echo "")

if [[ -z "$ENV_CONTENT" ]]; then
  echo "Error: .zbobr.env not found in $DOMAIN_PROJECT"
  echo "Domain project must have a .zbobr.env file with ZBOBR_FORK_OWNER defined"
  exit 1
fi

# Source the env content
eval "$ENV_CONTENT"

# Validate required configuration
if [[ -z "$ZBOBR_FORK_OWNER" ]]; then
  echo "Error: ZBOBR_FORK_OWNER not set in .zbobr.env"
  exit 1
fi

echo "Configuration loaded:"
echo "  ZBOBR_FORK_OWNER: $ZBOBR_FORK_OWNER"
echo "  ZBOBR_DEFAULT_MODEL: ${ZBOBR_DEFAULT_MODEL:-not set}"

# Create work directory
mkdir -p "copilot/projects"

# Clone target repository if not already cloned
if [[ ! -d "$WORK_DIR" ]]; then
  echo "Cloning $TARGET_REPO..."
  gh repo clone "$TARGET_REPO" "$WORK_DIR"
fi

cd "$WORK_DIR"

# Create fork if it doesn't exist
FORK_REPO="$ZBOBR_FORK_OWNER/$REPO_NAME"
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
