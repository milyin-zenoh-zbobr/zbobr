#!/bin/bash
set -euo pipefail

# Parse arguments
DRY_RUN=false
DOMAIN_PROJECT=""
FORK_OWNER=""

print_usage() {
    echo "Usage: $0 [--dry-run|-n] [--domain-project org/copilot-domain] [--fork-owner user-or-org] [--repo owner/repo]"
    echo ""
    echo "Options:"
    echo "  --dry-run            Show what would be done without making changes"
    echo "  --domain-project     Domain project repo (e.g., YoroolGui/copilot-zenoh)"
    echo "  --fork-owner         Where to create forks (user or org)"
    echo "  --repo               Repository to setup (for backward compatibility, same as --domain-project)"
}

# Bash 3-compatible array reader (macOS default bash)
read_lines() {
    local var_name="$1"
    local line
    eval "$var_name=()"
    while IFS= read -r line; do
        eval "$var_name+=(\"$line\")"
    done
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run|-n)
            DRY_RUN=true
            shift
            ;;
        --domain-project)
            DOMAIN_PROJECT="$2"
            shift 2
            ;;
        --fork-owner)
            FORK_OWNER="$2"
            shift 2
            ;;
        --repo)
            DOMAIN_PROJECT="$2"
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

# Define SCRIPT_DIR early for template path calculation
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Set REPO to domain project for lib.sh sourcing
if [[ -n "$DOMAIN_PROJECT" ]]; then
    REPO="$DOMAIN_PROJECT"
    export REPO
else
    # Default to orchestrator repo
    REPO="${REPO:-milyin/copilot}"
    export REPO
fi

# Initialize domain project if specified
if [[ -n "$DOMAIN_PROJECT" ]]; then
    echo "Domain Project: $DOMAIN_PROJECT"
    echo "Checking if domain project exists..."
    
    if ! gh repo view "$DOMAIN_PROJECT" >/dev/null 2>&1; then
        echo "Domain project does not exist. Creating..."
        if [[ "$DRY_RUN" == true ]]; then
            echo "DRY RUN: gh repo create $DOMAIN_PROJECT --public --initialize"
        else
            gh repo create "$DOMAIN_PROJECT" --public --initialize
            echo "Created domain project: $DOMAIN_PROJECT"
        fi
    else
        echo "Domain project exists."
    fi
    
    echo "Initializing domain project files..."
    ORCHESTRATOR_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"
    TEMPLATES_DIR="$ORCHESTRATOR_DIR/templates"
    
    if [[ -d "$TEMPLATES_DIR" ]]; then
        for template_file in "$TEMPLATES_DIR"/domain-*; do
            if [[ -f "$template_file" ]]; then
                target_name="${template_file##*/domain-}"
                echo "  Would create $target_name in $DOMAIN_PROJECT"
            fi
        done
    fi
    
    echo
fi

# Source common library functions (SCRIPT_DIR already defined above)
source "$SCRIPT_DIR/lib.sh"

if [[ "$DRY_RUN" == true ]]; then
    echo "Dry-run mode enabled. No changes will be applied."
    create_label() { echo "DRY RUN: create_label \"$1\" \"$2\" \"$3\""; }
    delete_label() { echo "DRY RUN: delete_label \"$1\""; }
    update_label() { echo "DRY RUN: update_label \"$1\" \"$2\" \"$3\""; }
    create_milestone() { echo "DRY RUN: create_milestone \"$1\" \"$2\""; }
    delete_milestone() { echo "DRY RUN: delete_milestone \"$1\""; }
    set_fork_owner() { echo "DRY RUN: set_fork_owner \"$1\""; }
fi

# Store fork owner in domain project configuration if specified
if [[ -n "$DOMAIN_PROJECT" ]] && [[ -n "$FORK_OWNER" ]]; then
    echo "Storing fork owner configuration: $FORK_OWNER"
    if [[ "$DRY_RUN" == true ]]; then
        echo "DRY RUN: Would store fork_owner=$FORK_OWNER in $DOMAIN_PROJECT/.copilot-config"
    else
        set_fork_owner "$FORK_OWNER"
        echo "Fork owner stored in .copilot-config"
    fi
    echo
fi

echo "Setting up repository: $REPO"
echo

# Check if repository exists before processing labels/milestones
REPO_EXISTS=true
if ! gh repo view "$REPO" >/dev/null 2>&1; then
    REPO_EXISTS=false
    echo "Note: Repository $REPO does not exist yet. Skipping label/milestone processing."
    echo
fi

if [[ "$REPO_EXISTS" == true ]]; then
# Get available models from copilot CLI
get_available_models() {
    local help_text
    local choices_block
    help_text=$(copilot help 2>/dev/null || copilot --help 2>/dev/null || true)
    choices_block=$(echo "$help_text" | awk '
        /--model <model>/ {flag=1}
        flag {print}
        flag && /\)/ {exit}
    ')
    if [[ -z "$choices_block" ]]; then
        return 0
    fi
    echo "$choices_block" \
        | tr '\n' ' ' \
        | sed -E 's/.*choices: *//; s/\).*//; s/"//g' \
        | tr ',' '\n' \
        | sed -E 's/^ *//; s/ *$//' \
        | grep -E '.+' || true
}

MODEL_LABEL_COLOR="bfd4f2"

# Process labels
echo "Processing labels..."

# Get existing labels
read_lines existing_labels < <(get_labels)

# Build desired labels list: all model:* labels + done label
declare -a desired_labels=()

# Add model labels
declare -a models=()
read_lines models < <(get_available_models)
for model in "${models[@]:-}"; do
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

# Update existing labels to desired color/description
echo "Updating existing labels:"
for label in "${desired_labels[@]}"; do
    if [[ "$label" =~ ^model:(.*)$ ]]; then
        model="${BASH_REMATCH[1]}"
        description="Use $model model"
        echo "  ~ $label"
        update_label "$label" "$MODEL_LABEL_COLOR" "$description"
    elif [[ "$label" == "done" ]]; then
        echo "  ~ $label"
        update_label "$label" "5319e7" "Issue implementation completed"
    fi
done

echo

# Process milestones
echo "Processing milestones..."

# Get existing milestones
read_lines existing_milestones < <(get_milestones)

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
        if [[ -z "$milestone" ]]; then
            continue
        fi
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
        if [[ -z "$milestone" ]]; then
            continue
        fi
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

fi  # Close REPO_EXISTS condition

echo
echo "✓ Repository setup complete"
