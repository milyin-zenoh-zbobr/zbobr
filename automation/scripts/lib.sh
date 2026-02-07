#!/bin/bash
# Common library functions for Copilot Agent Workflow scripts

# Load .zbobr.env if ZBOBR_DOMAIN_REPO is not already set
# (setup.sh sets it before sourcing, domain scripts load from file)
if [[ -z "$ZBOBR_DOMAIN_REPO" ]]; then
  if [[ -f ".zbobr.env" ]]; then
    source ".zbobr.env"
    # Remember the domain directory (where .zbobr.env lives)
    ZBOBR_DOMAIN_DIR="$(pwd)"
  else
    echo "Error: .zbobr.env not found in current directory" >&2
    echo "Scripts must be run from the domain project directory" >&2
    exit 1
  fi
else
  # If ZBOBR_DOMAIN_REPO was pre-set, assume we're in domain dir
  ZBOBR_DOMAIN_DIR="${ZBOBR_DOMAIN_DIR:-$(pwd)}"
fi
export ZBOBR_DOMAIN_REPO
export ZBOBR_FORK_OWNER
export ZBOBR_DEFAULT_MODEL
export ZBOBR_DOMAIN_DIR

# Set default workspace if not specified
ZBOBR_WORKSPACE="${ZBOBR_WORKSPACE:-$ZBOBR_DOMAIN_DIR/workspace}"
export ZBOBR_WORKSPACE

# Universal list reconciliation function (bash 3 compatible)
# Compares existing vs desired items and determines what to delete and create
# Usage: reconcile_lists "existing_array" "desired_array" "to_delete_array" "to_create_array"
reconcile_lists() {
  local existing_name=$1
  local desired_name=$2
  local delete_name=$3
  local create_name=$4

  eval "local -a existing=(\"\${${existing_name}[@]}\")"
  eval "local -a desired=(\"\${${desired_name}[@]}\")"

  local -a to_delete=()
  local -a to_create=()

  for item in "${existing[@]}"; do
    local found=false
    for wanted in "${desired[@]}"; do
      if [[ "$item" == "$wanted" ]]; then
        found=true
        break
      fi
    done
    if [[ "$found" == false ]]; then
      to_delete+=("$item")
    fi
  done

  for item in "${desired[@]}"; do
    local found=false
    for have in "${existing[@]}"; do
      if [[ "$item" == "$have" ]]; then
        found=true
        break
      fi
    done
    if [[ "$found" == false ]]; then
      to_create+=("$item")
    fi
  done

  eval "$delete_name=(\"\${to_delete[@]-}\")"
  eval "$create_name=(\"\${to_create[@]-}\")"
}

# Get all labels in a repository
# Usage: get_labels
get_labels() {
  gh label list --repo "$ZBOBR_DOMAIN_REPO" --json name --jq '.[].name'
}

# Get all milestones in a repository
# Usage: get_milestones
get_milestones() {
  gh api "repos/$ZBOBR_DOMAIN_REPO/milestones" --jq '.[].title'
}

# Create a label in a repository
# Usage: create_label "name" "color" "description"
create_label() {
  local name="$1"
  local color="$2"
  local description="$3"
  
  gh label create "$name" --description "$description" --color "$color" --repo "$ZBOBR_DOMAIN_REPO"
}

# Delete a label from a repository
# Usage: delete_label "name"
delete_label() {
  local name="$1"
  
  gh label delete "$name" --repo "$ZBOBR_DOMAIN_REPO" --yes
}

# Update a label in a repository
# Usage: update_label "name" "color" "description"
update_label() {
  local name="$1"
  local color="$2"
  local description="$3"

  gh label edit "$name" --description "$description" --color "$color" --repo "$ZBOBR_DOMAIN_REPO"
}

# Create a milestone in a repository
# Usage: create_milestone "title" "description"
create_milestone() {
  local title="$1"
  local description="$2"
  
  gh api "repos/$ZBOBR_DOMAIN_REPO/milestones" -f title="$title" -f description="$description" -f state="open"
}

# Delete a milestone from a repository
# Usage: delete_milestone "title"
delete_milestone() {
  local title="$1"
  
  local milestone_number=$(gh api "repos/$ZBOBR_DOMAIN_REPO/milestones" --jq ".[] | select(.title==\"$title\") | .number")
  if [[ -n "$milestone_number" ]]; then
    gh api "repos/$ZBOBR_DOMAIN_REPO/milestones/$milestone_number" -X DELETE
  fi
}

# Get issue labels
# Usage: get_issue_labels "issue_number"
get_issue_labels() {
  local issue_number="$1"
  
  gh issue view "$issue_number" --repo "$ZBOBR_DOMAIN_REPO" --json labels --jq '.labels[].name'
}

# Get milestone number by title
# Usage: get_milestone_number "milestone_title"
get_milestone_number() {
  local title="$1"
  
  gh api "repos/$ZBOBR_DOMAIN_REPO/milestones" --jq ".[] | select(.title==\"$title\") | .number"
}

# Get issue milestone
# Usage: get_issue_milestone "issue_number"
get_issue_milestone() {
  local issue_number="$1"
  
  gh issue view "$issue_number" --repo "$ZBOBR_DOMAIN_REPO" --json milestone --jq '.milestone.title // ""'
}

# Set issue milestone
# Usage: set_issue_milestone "issue_number" "milestone_title"
set_issue_milestone() {
  local issue_number="$1"
  local milestone_title="$2"
  
  local milestone_number=$(get_milestone_number "$milestone_title")
  
  if [[ -z "$milestone_number" ]]; then
    echo "Error: Milestone '$milestone_title' not found in $ZBOBR_DOMAIN_REPO" >&2
    return 1
  fi
  
  gh api "repos/$ZBOBR_DOMAIN_REPO/issues/$issue_number" -X PATCH -f milestone="$milestone_number" >/dev/null
}

# Add label to issue
# Usage: add_issue_label "issue_number" "label"
add_issue_label() {
  local issue_number="$1"
  local label="$2"
  
  gh issue edit "$issue_number" --repo "$ZBOBR_DOMAIN_REPO" --add-label "$label"
}

# Remove label from issue
# Usage: remove_issue_label "issue_number" "label"
remove_issue_label() {
  local issue_number="$1"
  local label="$2"
  
  gh issue edit "$issue_number" --repo "$ZBOBR_DOMAIN_REPO" --remove-label "$label"
}

# Check if issue has label
# Usage: has_issue_label "issue_number" "label"
# Returns: 0 if label exists, 1 if not
has_issue_label() {
  local issue_number="$1"
  local label="$2"

  local labels=$(get_issue_labels "$issue_number")

  for existing_label in $labels; do
    if [[ "$existing_label" == "$label" ]]; then
      return 0
    fi
  done

  return 1
}

# =============================================================================
# HIGH-LEVEL AGENT API
# These functions are exported for agents to use from any directory
# =============================================================================

# Get workspace directory for an issue
# Usage: get_issue_workdir <issue_number>
# Returns: Path to issue workspace (e.g., $ZBOBR_WORKSPACE/issue#123)
get_issue_workdir() {
  local issue_number="$1"

  if [[ -z "$issue_number" ]]; then
    echo "Error: get_issue_workdir requires issue_number" >&2
    return 1
  fi

  echo "$ZBOBR_WORKSPACE/issue#$issue_number"
}

# Get current issue number from working directory
# Usage: get_current_issue
# Returns: Issue number if in issue workspace, empty otherwise
get_current_issue() {
  local current_dir="$(pwd)"

  # Check if we're in an issue workspace directory
  if [[ "$current_dir" =~ $ZBOBR_WORKSPACE/issue#([0-9]+) ]]; then
    echo "${BASH_REMATCH[1]}"
  elif [[ "$current_dir" =~ issue#([0-9]+) ]]; then
    echo "${BASH_REMATCH[1]}"
  else
    echo ""
  fi
}

# Get GitHub issue URL
# Usage: get_issue_url [issue_number]
# If no issue_number provided, uses current issue from directory
# Returns: Full GitHub issue URL
get_issue_url() {
  local issue_number="${1:-$(get_current_issue)}"

  if [[ -z "$issue_number" ]]; then
    echo "Error: get_issue_url requires issue_number or must be in issue workspace" >&2
    return 1
  fi

  echo "https://github.com/$ZBOBR_DOMAIN_REPO/issues/$issue_number"
}

# Complete planning phase for an issue
# Usage: complete_planning [issue_number]
# If no issue_number provided, uses current issue from directory
# Sets milestone to PENDING (waiting for human approval)
complete_planning() {
  local issue_number="${1:-$(get_current_issue)}"

  if [[ -z "$issue_number" ]]; then
    echo "Error: complete_planning requires issue_number or must be in issue workspace" >&2
    return 1
  fi

  set_issue_milestone "$issue_number" "PENDING"
  echo "Issue #$issue_number: planning complete, awaiting approval" >&2
}

# Set issue done status
# Usage: set_issue_done [issue_number] <true|false>
# If only one arg provided, uses current issue from directory
# - true: sets milestone to PENDING and adds 'done' label
# - false: removes 'done' label (if present)
set_issue_done() {
  local issue_number
  local done

  if [[ $# -eq 1 ]]; then
    issue_number="$(get_current_issue)"
    done="$1"
  else
    issue_number="$1"
    done="$2"
  fi

  if [[ -z "$issue_number" ]]; then
    echo "Error: set_issue_done requires issue_number or must be in issue workspace" >&2
    return 1
  fi

  if [[ -z "$done" ]]; then
    echo "Error: set_issue_done requires done (true/false)" >&2
    return 1
  fi

  if [[ "$done" == "true" ]]; then
    set_issue_milestone "$issue_number" "PENDING"
    add_issue_label "$issue_number" "done"
    echo "Issue #$issue_number marked as done" >&2
  else
    # Remove done label if present (ignore error if not present)
    remove_issue_label "$issue_number" "done" 2>/dev/null || true
    echo "Issue #$issue_number: done label removed" >&2
  fi
}

# Clone and fork a target repository for issue implementation
# Usage: clone_target <target_repo> [issue_number]
# If no issue_number provided, uses current issue from directory
# Clones into current issue workspace
# Returns: Path to the cloned repository
clone_target() {
  local target_repo="$1"
  local issue_number="${2:-$(get_current_issue)}"

  if [[ -z "$target_repo" ]]; then
    echo "Error: clone_target requires target_repo" >&2
    return 1
  fi

  if [[ -z "$issue_number" ]]; then
    echo "Error: clone_target requires issue_number or must be in issue workspace" >&2
    return 1
  fi

  if [[ -z "$ZBOBR_FORK_OWNER" ]]; then
    echo "Error: ZBOBR_FORK_OWNER not set" >&2
    return 1
  fi

  local repo_name="${target_repo#*/}"
  local issue_workdir="$(get_issue_workdir "$issue_number")"
  local work_dir="$issue_workdir/$repo_name"

  # Create issue workspace
  mkdir -p "$issue_workdir"

  # Clone target repository if not already cloned
  if [[ ! -d "$work_dir" ]]; then
    echo "Cloning $target_repo into $work_dir..." >&2
    gh repo clone "$target_repo" "$work_dir"
  fi

  # Create fork if it doesn't exist
  local fork_repo="$ZBOBR_FORK_OWNER/$repo_name"
  if ! gh repo view "$fork_repo" >/dev/null 2>&1; then
    echo "Creating fork to $ZBOBR_FORK_OWNER..." >&2
    gh repo fork "$target_repo" --org "$ZBOBR_FORK_OWNER" --clone=false
    sleep 2  # Wait for fork to be ready
  fi

  # Add fork as remote (run in subshell to preserve cwd)
  (
    cd "$work_dir"
    if ! git remote get-url fork >/dev/null 2>&1; then
      echo "Adding fork remote..." >&2
      git remote add fork "https://github.com/$fork_repo.git"
    fi

    # Create feature branch
    local branch_name="fix${issue_number}/implementation"
    if ! git rev-parse --verify "$branch_name" >/dev/null 2>&1; then
      echo "Creating branch $branch_name..." >&2
      git checkout -b "$branch_name"
    else
      echo "Checking out existing branch $branch_name..." >&2
      git checkout "$branch_name"
    fi
  )

  echo "$work_dir"
}

# =============================================================================
# FUNCTION EXPORT HELPERS
# Call these to make functions available to child processes (copilot agents)
# =============================================================================

# Export functions needed by Planner agent
export_planner_functions() {
  # High-level API
  export -f get_issue_workdir
  export -f get_current_issue
  export -f get_issue_url
  export -f complete_planning

  # Internal dependencies
  export -f set_issue_milestone
  export -f get_milestone_number
}

# Export functions needed by Worker agent
export_worker_functions() {
  # High-level API
  export -f get_issue_workdir
  export -f get_current_issue
  export -f get_issue_url
  export -f set_issue_done
  export -f clone_target

  # Internal dependencies
  export -f get_issue_labels
  export -f set_issue_milestone
  export -f get_milestone_number
  export -f add_issue_label
  export -f remove_issue_label
}
