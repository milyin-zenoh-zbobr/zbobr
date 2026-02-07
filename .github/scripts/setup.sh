#!/bin/bash
set -euo pipefail

# Source common library functions
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"

echo "Setting up repository: $REPO"
echo

# Get available models from copilot CLI
get_available_models() {
    local help_text
    local choices_line
    help_text=$(copilot help 2>/dev/null || copilot --help 2>/dev/null || true)
    choices_line=$(echo "$help_text" | grep -E "--model <model>" | head -n 1 || true)
    echo "$choices_line" \
        | sed -E 's/.*choices: //; s/[)]$//; s/"//g' \
        | tr ',' '\n' \
        | sed -E 's/^ *//; s/ *$//' \
        | grep -E '.+' || true
}

MODEL_LABEL_COLOR="bfd4f2"

# Process labels
echo "Processing labels..."

# Get existing labels
mapfile -t existing_labels < <(get_labels)

# Build desired labels list: all model:* labels + done label
declare -a desired_labels=()

# Add model labels
mapfile -t models < <(get_available_models)
for model in "${models[@]}"; do
    if [[ -n "$model" ]]; then
        desired_labels+=("model:$model")
    fi
done

# Add done label
desired_labels+=("done")

# Reconcile labels (find what to delete and what to create)
declare -a labels_to_delete=()
declare -a labels_to_create=()
reconcile_lists existing_labels desired_labels labels_to_delete labels_to_create

# Delete extra labels
if [[ ${#labels_to_delete[@]} -gt 0 ]]; then
    echo "Deleting extra labels:"
    for label in "${labels_to_delete[@]}"; do
        if [[ "$label" =~ ^model: ]] || [[ "$label" == "done" ]]; then
            echo "  - $label"
            delete_label "$label"
        fi
    done
else
    echo "No extra labels to delete"
fi

# Create missing labels
if [[ ${#labels_to_create[@]} -gt 0 ]]; then
    echo "Creating missing labels:"
    for label in "${labels_to_create[@]}"; do
        if [[ "$label" =~ ^model:(.*)$ ]]; then
            model="${BASH_REMATCH[1]}"
            description="Use $model model"
            echo "  + $label"
            create_label "$label" "$MODEL_LABEL_COLOR" "$description"
        elif [[ "$label" == "done" ]]; then
            echo "  + $label"
            create_label "$label" "5319e7" "Issue implementation completed"
        fi
    done
else
    echo "No missing labels to create"
fi

echo

# Process milestones
echo "Processing milestones..."

# Get existing milestones
mapfile -t existing_milestones < <(get_milestones)

# Desired milestones
declare -a desired_milestones=(
    "PLANNING"
    "PENDING"
    "READY"
    "WORKING"
)

# Reconcile milestones
declare -a milestones_to_delete=()
declare -a milestones_to_create=()
reconcile_lists existing_milestones desired_milestones milestones_to_delete milestones_to_create

# Delete extra milestones
if [[ ${#milestones_to_delete[@]} -gt 0 ]]; then
    echo "Deleting extra milestones:"
    for milestone in "${milestones_to_delete[@]}"; do
        echo "  - $milestone"
        delete_milestone "$milestone"
    done
else
    echo "No extra milestones to delete"
fi

# Create missing milestones
if [[ ${#milestones_to_create[@]} -gt 0 ]]; then
    echo "Creating missing milestones:"
    for milestone in "${milestones_to_create[@]}"; do
        case "$milestone" in
            PLANNING)
                description="Issue is being planned by Manager agent"
                ;;
            PENDING)
                description="Issue plan is complete, awaiting human review or implementation is done"
                ;;
            READY)
                description="Issue is approved and ready for Worker agent"
                ;;
            WORKING)
                description="Issue is being implemented by Worker agent"
                ;;
            *)
                description=""
                ;;
        esac
        echo "  + $milestone"
        create_milestone "$milestone" "$description"
    done
else
    echo "No missing milestones to create"
fi

echo
echo "✓ Repository setup complete"
