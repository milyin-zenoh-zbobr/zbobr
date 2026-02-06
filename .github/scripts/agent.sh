#!/bin/bash
# Agent CLI wrapper — invoke Manager or Worker agents

set -e

AGENT_TYPE="${1:-manager}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AGENTS_DIR="$SCRIPT_DIR/../agents"

case "$AGENT_TYPE" in
  manager)
    echo "Launching Manager Agent via Copilot CLI..."
    PROMPT="${2:-Follow the instructions in .github/agents/manager.md and start processing issues.}"
    copilot --agent manager -i "$PROMPT"
    ;;
  worker)
    echo "Launching Worker Agent via Copilot CLI..."
    # Worker expects issue number and repo to build a default prompt.
    ISSUE="${2:-}"
    REPO="${3:-}"
    MODEL="${4:-gpt-5.2-codex}"

    if [[ -z "$ISSUE" || -z "$REPO" ]]; then
      echo "Error: Worker requires issue number and repo"
      echo "Usage: $0 worker <issue_number> <repo> [model]"
      exit 1
    fi

    PROMPT="Fix issue #$ISSUE in $REPO. Follow the instructions in .github/agents/worker.md."
    copilot --agent worker --model "$MODEL" -i "$PROMPT"
    ;;
  *)
    echo "Unknown agent: $AGENT_TYPE"
    echo "Available agents: manager, worker"
    exit 1
    ;;
esac
