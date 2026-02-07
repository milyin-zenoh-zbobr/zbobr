#!/bin/bash
# Common library functions for Copilot Agent Workflow scripts

# Default repository (can be overridden by scripts before sourcing)
: "${REPO:=milyin/copilot}"
export REPO

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
  gh label list --repo "$REPO" --json name --jq '.[].name'
}

# Get all milestones in a repository
# Usage: get_milestones
get_milestones() {
  gh api "repos/$REPO/milestones" --jq '.[].title'
}

# Create a label in a repository
# Usage: create_label "name" "color" "description"
create_label() {
  local name="$1"
  local color="$2"
  local description="$3"
  
  gh label create "$name" --description "$description" --color "$color" --repo "$REPO"
}

# Delete a label from a repository
# Usage: delete_label "name"
delete_label() {
  local name="$1"
  
  gh label delete "$name" --repo "$REPO" --yes
}

# Update a label in a repository
# Usage: update_label "name" "color" "description"
update_label() {
  local name="$1"
  local color="$2"
  local description="$3"

  gh label edit "$name" --description "$description" --color "$color" --repo "$REPO"
}

# Create a milestone in a repository
# Usage: create_milestone "title" "description"
create_milestone() {
  local title="$1"
  local description="$2"
  
  gh api "repos/$REPO/milestones" -f title="$title" -f description="$description" -f state="open"
}

# Delete a milestone from a repository
# Usage: delete_milestone "title"
delete_milestone() {
  local title="$1"
  
  local milestone_number=$(gh api "repos/$REPO/milestones" --jq ".[] | select(.title==\"$title\") | .number")
  if [[ -n "$milestone_number" ]]; then
    gh api "repos/$REPO/milestones/$milestone_number" -X DELETE
  fi
}

# Get issue labels
# Usage: get_issue_labels "issue_number"
get_issue_labels() {
  local issue_number="$1"
  
  gh issue view "$issue_number" --repo "$REPO" --json labels --jq '.labels[].name'
}

# Extract model from issue labels (looks for model: prefix)
# Usage: extract_model_from_labels "issue_number" "default_model"
extract_model_from_labels() {
  local issue_number="$1"
  local default_model="${2:-gpt-5-mini}"
  
  local labels=$(get_issue_labels "$issue_number")
  
  for label in $labels; do
    if [[ "$label" =~ ^model: ]]; then
      echo "${label#model:}"
      return
    fi
  done
  
  echo "$default_model"
}
# Get configuration value from domain project's .copilot-config file
# Usage: get_config "key" ["default_value"]
get_config() {
  local key="$1"
  local default="${2:-}"
  
  # Try to get config from repository
  local config_content=$(gh api "repos/$REPO/contents/.copilot-config" --jq '.content' 2>/dev/null | base64 -d 2>/dev/null || echo "")
  
  if [[ -n "$config_content" ]]; then
    local value=$(echo "$config_content" | grep "^${key}=" | cut -d'=' -f2-)
    if [[ -n "$value" ]]; then
      echo "$value"
      return
    fi
  fi
  
  echo "$default"
}

# Set configuration value in domain project's .copilot-config file
# Usage: set_config "key" "value"
set_config() {
  local key="$1"
  local value="$2"
  
  # Get existing config or start fresh
  local config_content=$(gh api "repos/$REPO/contents/.copilot-config" --jq '.content' 2>/dev/null | base64 -d 2>/dev/null || echo "")
  local sha=$(gh api "repos/$REPO/contents/.copilot-config" --jq '.sha' 2>/dev/null || echo "")
  
  # Update or add the key
  local new_config=""
  local key_found=false
  
  while IFS= read -r line; do
    if [[ "$line" =~ ^${key}= ]]; then
      new_config="${new_config}${key}=${value}"$'\n'
      key_found=true
    elif [[ -n "$line" ]]; then
      new_config="${new_config}${line}"$'\n'
    fi
  done <<< "$config_content"
  
  if [[ "$key_found" == false ]]; then
    new_config="${new_config}${key}=${value}"$'\n'
  fi
  
  # Create or update the file
  local encoded_content=$(echo -n "$new_config" | base64)
  
  if [[ -n "$sha" ]]; then
    # Update existing file
    gh api "repos/$REPO/contents/.copilot-config" -X PUT \
      -f message="Update $key configuration" \
      -f content="$encoded_content" \
      -f sha="$sha" >/dev/null
  else
    # Create new file
    gh api "repos/$REPO/contents/.copilot-config" -X PUT \
      -f message="Initialize copilot configuration" \
      -f content="$encoded_content" >/dev/null
  fi
}

# Get fork owner from configuration
# Usage: get_fork_owner ["default_value"]
get_fork_owner() {
  local default="${1:-}"
  get_config "fork_owner" "$default"
}

# Set fork owner in configuration
# Usage: set_fork_owner "owner"
set_fork_owner() {
  local owner="$1"
  set_config "fork_owner" "$owner"
}
# Get milestone number by title
# Usage: get_milestone_number "milestone_title"
get_milestone_number() {
  local title="$1"
  
  gh api "repos/$REPO/milestones" --jq ".[] | select(.title==\"$title\") | .number"
}

# Get issue milestone
# Usage: get_issue_milestone "issue_number"
get_issue_milestone() {
  local issue_number="$1"
  
  gh issue view "$issue_number" --repo "$REPO" --json milestone --jq '.milestone.title // ""'
}

# Set issue milestone
# Usage: set_issue_milestone "issue_number" "milestone_title"
set_issue_milestone() {
  local issue_number="$1"
  local milestone_title="$2"
  
  local milestone_number=$(get_milestone_number "$milestone_title")
  
  if [[ -z "$milestone_number" ]]; then
    echo "Error: Milestone '$milestone_title' not found in $REPO" >&2
    return 1
  fi
  
  gh api "repos/$REPO/issues/$issue_number" -X PATCH -f milestone="$milestone_number" >/dev/null
}

# Add label to issue
# Usage: add_issue_label "issue_number" "label"
add_issue_label() {
  local issue_number="$1"
  local label="$2"
  
  gh issue edit "$issue_number" --repo "$REPO" --add-label "$label"
}

# Remove label from issue
# Usage: remove_issue_label "issue_number" "label"
remove_issue_label() {
  local issue_number="$1"
  local label="$2"
  
  gh issue edit "$issue_number" --repo "$REPO" --remove-label "$label"
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
