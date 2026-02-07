#!/bin/bash
# Spawn a Worker agent to handle a READY issue

set -e

# Parse arguments
ISSUE=""
MODEL="gpt-5-mini"

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
    *)
      echo "Unknown argument: $1"
      echo "Usage: $0 --issue <issue_number> [--model <model_name>]"
      exit 1
      ;;
  esac
done

# Validate required arguments
if [[ -z "$ISSUE" ]]; then
  echo "Error: --issue is required"
  echo "Usage: $0 --issue <issue_number> [--model <model_name>]"
  exit 1
fi

# Build prompt for Worker
PROMPT="Fix issue https://github.com/milyin/copilot/issues/$ISSUE. Follow the instructions in automation/agents/worker.md."

# Launch Worker agent in background
echo "Spawning Worker agent for issue #$ISSUE with model $MODEL..."
copilot --agent worker --model "$MODEL" -i "$PROMPT" --allow-all &

WORKER_PID=$!
echo "Worker agent started (PID: $WORKER_PID)"
echo "Worker is running in background. Check logs at ~/.copilot/logs/"
