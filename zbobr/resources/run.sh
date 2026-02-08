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

# Run zbobr, passing all arguments through
exec zbobr "$@"
