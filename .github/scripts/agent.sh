#!/bin/bash
# Agent CLI wrapper — invoke Manager or Worker agents

set -e

AGENT_TYPE="${1:-manager}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AGENTS_DIR="$SCRIPT_DIR/../agents"

case "$AGENT_TYPE" in
  manager)
    echo "Launching Manager Agent..."
    cat "$AGENTS_DIR/manager.md"
    ;;
  worker)
    echo "Launching Worker Agent..."
    # Worker expects additional arguments: --issue, --repo, --model
    ISSUE="${2:-}"
    REPO="${3:-}"
    MODEL="${4:-GPT-5-Mini}"
    
    if [[ -z "$ISSUE" || -z "$REPO" ]]; then
      echo "Error: Worker requires --issue and --repo arguments"
      echo "Usage: $0 worker <issue_number> <repo> [model]"
      exit 1
    fi
    
    echo "Worker Agent for Issue #$ISSUE in $REPO (Model: $MODEL)"
    cat "$AGENTS_DIR/worker.md"
    ;;
  *)
    echo "Unknown agent: $AGENT_TYPE"
    echo "Available agents: manager, worker"
    exit 1
    ;;
esac
