#!/bin/bash
# Process issues in a loop: run Planner for PLANNING, Worker for READY
# Must be run from domain project directory (with .zbobr.env)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"

POLL_SECONDS=60
MODEL="${ZBOBR_DEFAULT_MODEL:-gpt-5-mini}"

print_usage() {
    echo "Usage: $0 [--interval seconds] [--model model_name]"
    echo ""
    echo "Must be run from domain project directory (with .zbobr.env)"
    echo ""
    echo "Options:"
    echo "  --interval    Poll interval in seconds (default: 60)"
    echo "  --model       AI model to use (default: \$ZBOBR_DEFAULT_MODEL or gpt-5-mini)"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --interval)
            POLL_SECONDS="$2"
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

echo "Issue processor started for $ZBOBR_DOMAIN_REPO"
echo "  Interval: ${POLL_SECONDS}s"
echo "  Model: $MODEL"
echo "  Workspace: $ZBOBR_WORKSPACE"
echo ""

# Get first issue with given milestone
# Usage: get_first_issue <milestone>
# Returns: issue number or empty
get_first_issue() {
    local milestone="$1"
    gh issue list --repo "$ZBOBR_DOMAIN_REPO" --state open --limit 1 \
        --search "milestone:$milestone" \
        --json number --jq '.[0].number // empty'
}

# Process one iteration
process_issues() {
    local issue

    # Check for PLANNING issues first
    issue=$(get_first_issue "PLANNING")
    if [[ -n "$issue" ]]; then
        echo "Found PLANNING issue #$issue - running Planner agent..."
        "$SCRIPT_DIR/agent.sh" planner "$issue" "$MODEL" || {
            echo "Planner agent failed for issue #$issue"
        }
        return 0
    fi

    # Check for READY issues
    issue=$(get_first_issue "READY")
    if [[ -n "$issue" ]]; then
        echo "Found READY issue #$issue - spawning Worker..."

        # worker.sh handles subtask check and sets milestone to WORKING
        "$SCRIPT_DIR/worker.sh" --issue "$issue" --model "$MODEL" || {
            echo "Worker failed for issue #$issue"
        }
        return 0
    fi

    # No issues to process
    return 1
}

while true; do
    if process_issues; then
        echo "Agent completed. Checking for more issues..."
    else
        echo "No processable issues. Sleeping ${POLL_SECONDS}s..."
        sleep "$POLL_SECONDS"
    fi
done
