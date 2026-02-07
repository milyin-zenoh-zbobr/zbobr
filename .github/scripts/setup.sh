#!/bin/bash
# Setup script for Copilot Agent Workflow
# Creates required labels and milestones in milyin/copilot repository
# Idempotent: safe to run multiple times

set -e

REPO="milyin/copilot"

echo "Setting up Copilot Agent Workflow for $REPO..."

# Function to create label if it doesn't exist
create_label() {
  local name="$1"
  local description="$2"
  local color="$3"
  
  if gh label list --repo "$REPO" | grep -q "^${name}[[:space:]]"; then
    echo "  Label '$name' already exists"
  else
    gh label create "$name" --description "$description" --color "$color" --repo "$REPO"
    echo "  Created label '$name'"
  fi
}

# Function to create milestone if it doesn't exist
create_milestone() {
  local title="$1"
  local description="$2"
  
  if gh api "repos/$REPO/milestones" --jq '.[].title' | grep -q "^${title}$"; then
    echo "  Milestone '$title' already exists"
  else
    gh api "repos/$REPO/milestones" -f title="$title" -f description="$description" -f state="open" > /dev/null
    echo "  Created milestone '$title'"
  fi
}

echo ""
echo "Creating labels..."

# Model labels
create_label "model:gpt-5-mini" "Use GPT-5 Mini model (free tier)" "0E8A16"
create_label "model:gpt-5" "Use GPT-5 model" "1D76DB"
create_label "model:gpt-5.2-codex" "Use GPT-5.2 Codex model" "0052CC"
create_label "model:claude-sonnet-4.5" "Use Claude Sonnet 4.5 model" "5319E7"
create_label "model:claude-opus-4.5" "Use Claude Opus 4.5 model" "7B16FF"

# Status labels
create_label "done" "Task completed, awaiting review" "00FF00"

echo ""
echo "Creating milestones..."

# Stage milestones
create_milestone "PLANNING" "Manager creates implementation plan"
create_milestone "PENDING" "Awaiting human review and approval"
create_milestone "READY" "Ready for Worker to implement"
create_milestone "WORKING" "Worker is implementing the issue"

echo ""
echo "✓ Setup complete!"
echo ""
echo "Next steps:"
echo "  1. Create issues with milestone 'PLANNING'"
echo "  2. Run: copilot --agent manager -i \"Process issues using the manager workflow.\""
