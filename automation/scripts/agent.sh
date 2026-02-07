#!/bin/bash
# Agent CLI wrapper — invoke Manager or Worker agents
# Must be run from domain project directory (with .zbobr.env)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"

print_usage() {
  echo "Usage: $0 <agent_type> [prompt]"
  echo "       $0 worker <issue_number> [model]"
  echo ""
  echo "Must be run from domain project directory (with .zbobr.env)"
  echo ""
  echo "Agent types: manager, worker"
}

AGENT_TYPE="${1:-}"

if [[ -z "$AGENT_TYPE" ]] || [[ "$AGENT_TYPE" == "-h" ]] || [[ "$AGENT_TYPE" == "--help" ]]; then
  print_usage
  exit 0
fi

shift

case "$AGENT_TYPE" in
  manager)
    echo "Launching Manager Agent via Copilot CLI..."
    echo "Domain: $ZBOBR_DOMAIN_REPO"
    echo "Domain dir: $ZBOBR_DOMAIN_DIR"

    # Export functions for the agent to use from any directory
    export_manager_functions

    PROMPT="${1:-Follow the instructions in automation/agents/manager.md and start processing issues.}"
    copilot --agent manager -i "$PROMPT"
    ;;
  worker)
    echo "Launching Worker Agent via Copilot CLI..."
    echo "Domain: $ZBOBR_DOMAIN_REPO"
    echo "Domain dir: $ZBOBR_DOMAIN_DIR"
    ISSUE="${1:-}"
    MODEL="${2:-${ZBOBR_DEFAULT_MODEL:-gpt-5-mini}}"

    if [[ -z "$ISSUE" ]]; then
      echo "Error: Worker requires issue number"
      echo "Usage: $0 worker <issue_number> [model]"
      exit 1
    fi

    # Export functions for the agent to use from any directory
    export_worker_functions

    PROMPT="Fix issue https://github.com/$ZBOBR_DOMAIN_REPO/issues/$ISSUE. Follow the instructions in automation/agents/worker.md."
    copilot --agent worker --model "$MODEL" -i "$PROMPT"
    ;;
  *)
    echo "Unknown agent: $AGENT_TYPE"
    echo "Available agents: manager, worker"
    exit 1
    ;;
esac
