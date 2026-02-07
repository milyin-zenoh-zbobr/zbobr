#!/bin/bash
# Spawn a Worker agent to handle a READY issue
# Must be run from domain project directory (with .zbobr.env)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"

# Parse arguments
ISSUE=""
MODEL=""

print_usage() {
  echo "Usage: $0 --issue <issue_number> [--model <model_name>]"
  echo ""
  echo "Must be run from domain project directory (with .zbobr.env)"
  echo ""
  echo "Required:"
  echo "  --issue              Issue number to work on"
  echo ""
  echo "Options:"
  echo "  --model              AI model to use (default: \$ZBOBR_DEFAULT_MODEL or gpt-5-mini)"
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

# Validate required arguments
if [[ -z "$ISSUE" ]]; then
  echo "Error: --issue is required"
  print_usage
  exit 1
fi

# Use model from argument, env, or default
MODEL="${MODEL:-${ZBOBR_DEFAULT_MODEL:-gpt-5-mini}}"

# Build prompt for Worker
PROMPT="Fix issue https://github.com/$ZBOBR_DOMAIN_REPO/issues/$ISSUE. Follow the instructions in automation/agents/worker.md."

# Launch Worker agent in background
echo "Spawning Worker agent for issue #$ISSUE with model $MODEL..."
echo "Domain: $ZBOBR_DOMAIN_REPO"
copilot --agent worker --model "$MODEL" -i "$PROMPT" --allow-all &

WORKER_PID=$!
echo "Worker agent started (PID: $WORKER_PID)"
echo "Worker is running in background. Check logs at ~/.copilot/logs/"
