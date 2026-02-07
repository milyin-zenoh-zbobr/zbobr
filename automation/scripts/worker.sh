#!/bin/bash
# Spawn a Worker agent in background
# Must be run from domain project directory (with .zbobr.env)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"

ISSUE=""
MODEL=""

print_usage() {
  echo "Usage: $0 --issue <issue_number> [--model <model_name>]"
  echo ""
  echo "Spawns a worker agent in the background for the given issue."
  echo "Must be run from domain project directory (with .zbobr.env)"
  echo ""
  echo "Required:"
  echo "  --issue    Issue number to work on"
  echo ""
  echo "Options:"
  echo "  --model    AI model to use (default: \$ZBOBR_DEFAULT_MODEL or gpt-5-mini)"
}

while [[ $# -gt 0 ]]; do
  case $1 in
    --issue)
      ISSUE="$2"
      shift 2
      ;;
    --model)
      MODEL="$2"
      shift 2
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

if [[ -z "$ISSUE" ]]; then
  echo "Error: --issue is required"
  print_usage
  exit 1
fi

MODEL="${MODEL:-${ZBOBR_DEFAULT_MODEL:-gpt-5-mini}}"

# Set milestone to WORKING
set_issue_milestone "$ISSUE" "WORKING"

echo "Spawning Worker agent for issue #$ISSUE..."
echo "  Model: $MODEL"
echo "  Workspace: $(get_issue_workdir "$ISSUE")"

# Run agent.sh in background
"$SCRIPT_DIR/agent.sh" worker "$ISSUE" "$MODEL" &

WORKER_PID=$!
echo "Worker agent started (PID: $WORKER_PID)"
echo "Check logs at ~/.copilot/logs/"
