#!/bin/bash
# Run Manager agent in a loop when processable milestones exist

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="${REPO:-milyin/copilot}"
POLL_SECONDS=60

print_usage() {
    echo "Usage: $0 [--repo owner/repo] [--interval seconds]"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --repo)
            REPO="$2"
            shift 2
            ;;
        --interval)
            POLL_SECONDS="$2"
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

echo "Manager loop started for $REPO (interval: ${POLL_SECONDS}s)"

has_processable_issues() {
    local count
    count=$(gh issue list --repo "$REPO" --state open --limit 1 \
        --search "milestone:PLANNING OR milestone:READY" \
        --json number --jq 'length')
    [[ "$count" -gt 0 ]]
}

while true; do
    if has_processable_issues; then
        echo "Processable issues found. Running Manager agent..."
        "$SCRIPT_DIR/agent.sh" manager "Follow the instructions in .github/agents/manager.md and start processing issues."
    else
        echo "No processable issues. Sleeping ${POLL_SECONDS}s..."
    fi
    sleep "$POLL_SECONDS"
done
