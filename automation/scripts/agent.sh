#!/bin/bash
# Agent CLI wrapper — invoke Planner or Worker agents
# Must be run from domain project directory (with .zbobr.env)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"

print_usage() {
  echo "Usage: $0 <agent_type> <issue_number> [model]"
  echo ""
  echo "Must be run from domain project directory (with .zbobr.env)"
  echo ""
  echo "Agent types:"
  echo "  planner  - Investigate issue and create implementation plan"
  echo "  worker   - Implement issue according to plan"
  echo ""
  echo "Arguments:"
  echo "  issue_number  - GitHub issue number to work on"
  echo "  model         - AI model to use (default: \$ZBOBR_DEFAULT_MODEL or gpt-5-mini)"
}

AGENT_TYPE="${1:-}"
ISSUE="${2:-}"
MODEL="${3:-${ZBOBR_DEFAULT_MODEL:-gpt-5-mini}}"

if [[ -z "$AGENT_TYPE" ]] || [[ "$AGENT_TYPE" == "-h" ]] || [[ "$AGENT_TYPE" == "--help" ]]; then
  print_usage
  exit 0
fi

if [[ -z "$ISSUE" ]]; then
  echo "Error: Issue number is required"
  print_usage
  exit 1
fi

# Create issue workspace
ISSUE_WORKDIR="$(get_issue_workdir "$ISSUE")"
mkdir -p "$ISSUE_WORKDIR"

ISSUE_URL="$(get_issue_url "$ISSUE")"

echo "Agent: $AGENT_TYPE"
echo "Issue: #$ISSUE ($ISSUE_URL)"
echo "Model: $MODEL"
echo "Domain: $ZBOBR_DOMAIN_REPO"
echo "Workspace: $ISSUE_WORKDIR"
echo ""

case "$AGENT_TYPE" in
  planner)
    # Export functions for the agent
    export_planner_functions

    PROMPT="Investigate issue $ISSUE_URL and create an implementation plan. Follow the instructions in automation/agents/planner.md."

    # Run agent from issue workspace
    cd "$ISSUE_WORKDIR"
    copilot --agent planner --model "$MODEL" -i "$PROMPT"
    ;;
  worker)
    # Export functions for the agent
    export_worker_functions

    PROMPT="Implement issue $ISSUE_URL according to the plan. Follow the instructions in automation/agents/worker.md."

    # Run agent from issue workspace
    cd "$ISSUE_WORKDIR"
    copilot --agent worker --model "$MODEL" -i "$PROMPT"
    ;;
  *)
    echo "Unknown agent: $AGENT_TYPE"
    echo "Available agents: planner, worker"
    exit 1
    ;;
esac
