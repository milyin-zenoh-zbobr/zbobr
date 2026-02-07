#!/bin/bash
# Common library functions for Copilot Agent Workflow scripts

# Universal list reconciliation function
# Compares existing vs desired items and determines what to delete and create
# Usage: reconcile_lists "existing_array" "desired_array" "to_delete_array" "to_create_array"
reconcile_lists() {
  local -n existing_ref=$1
  local -n desired_ref=$2
  local -n to_delete_ref=$3
  local -n to_create_ref=$4
  
  # Start with all existing items marked for deletion
  to_delete_ref=("${existing_ref[@]}")
  
  # Start with all desired items marked for creation
  to_create_ref=("${desired_ref[@]}")
  
  # Find intersection and remove from both lists
  local -A existing_map
  for item in "${existing_ref[@]}"; do
    existing_map["$item"]=1
  done
  
  local -a new_to_create=()
  for item in "${desired_ref[@]}"; do
    if [[ -n "${existing_map[$item]}" ]]; then
      # Item exists in both - remove from to_delete
      local -a new_to_delete=()
      for del_item in "${to_delete_ref[@]}"; do
        [[ "$del_item" != "$item" ]] && new_to_delete+=("$del_item")
      done
      to_delete_ref=("${new_to_delete[@]}")
    else
      # Item doesn't exist - keep in to_create
      new_to_create+=("$item")
    fi
  done
  to_create_ref=("${new_to_create[@]}")
}

# Get all labels in a repository
# Usage: get_repo_labels "owner/repo"
get_repo_labels() {
  local repo="$1"
  gh label list --repo "$repo" --json name --jq '.[].name'
}

# Get all milestones in a repository
# Usage: get_repo_milestones "owner/repo"
get_repo_milestones() {
  local repo="$1"
  gh api "repos/$repo/milestones" --jq '.[].title'
}

# Create a label in a repository
# Usage: create_repo_label "owner/repo" "name" "description" "color"
create_repo_label() {
  local repo="$1"
  local name="$2"
  local description="$3"
  local color="$4"
  
  gh label create "$name" --description "$description" --color "$color" --repo "$repo"
}

# Delete a label from a repository
# Usage: delete_repo_label "owner/repo" "name"
delete_repo_label() {
  local repo="$1"
  local name="$2"
  
  gh label delete "$name" --repo "$repo" --yes
}

# Create a milestone in a repository
# Usage: create_repo_milestone "owner/repo" "title" "description"
create_repo_milestone() {
  local repo="$1"
  local title="$2"
  local description="$3"
  
  gh api "repos/$repo/milestones" -f title="$title" -f description="$description" -f state="open" > /dev/null
}

# Delete a milestone from a repository
# Usage: delete_repo_milestone "owner/repo" "title"
delete_repo_milestone() {
  local repo="$1"
  local title="$2"
  
  local milestone_number=$(gh api "repos/$repo/milestones" --jq ".[] | select(.title==\"$title\") | .number")
  if [[ -n "$milestone_number" ]]; then
    gh api "repos/$repo/milestones/$milestone_number" -X DELETE > /dev/null
  fi
}

# Get issue labels
# Usage: get_issue_labels "owner/repo" "issue_number"
get_issue_labels() {
  local repo="$1"
  local issue_number="$2"
  
  gh issue view "$issue_number" --repo "$repo" --json labels --jq '.labels[].name'
}

# Extract model from issue labels (looks for model: prefix)
# Usage: extract_model_from_labels "owner/repo" "issue_number" "default_model"
extract_model_from_labels() {
  local repo="$1"
  local issue_number="$2"
  local default_model="${3:-gpt-5-mini}"
  
  local labels=$(get_issue_labels "$repo" "$issue_number")
  
  for label in $labels; do
    if [[ "$label" =~ ^model: ]]; then
      echo "${label#model:}"
      return
    fi
  done
  
  echo "$default_model"
}
