#!/usr/bin/env bash
# Launch zbobr for this domain project.
# Source the env file and run zbobr with the given arguments.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Load configuration
if [[ -f "$SCRIPT_DIR/.zbobr.env" ]]; then
    set -a
    source "$SCRIPT_DIR/.zbobr.env"
    set +a
elif [[ -f "$SCRIPT_DIR/zbobr.env" ]]; then
    set -a
    source "$SCRIPT_DIR/zbobr.env"
    set +a
else
    echo "Error: No .zbobr.env or zbobr.env found in $SCRIPT_DIR" >&2
    exit 1
fi

# Set GH_TOKEN from gh CLI if not already defined
if [[ -z "${GH_TOKEN:-}" ]]; then
    if command -v gh &> /dev/null; then
        GH_TOKEN=$(gh auth token 2>/dev/null) || true
        if [[ -n "$GH_TOKEN" ]]; then
            export GH_TOKEN
        else
            echo "Warning: gh CLI found but not authenticated. Run 'gh auth login' first." >&2
        fi
    else
        echo "Warning: GH_TOKEN not set and gh CLI not found. Authentication may fail." >&2
    fi
fi

# Run zbobr, passing all arguments through
exec zbobr "$@"
