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
