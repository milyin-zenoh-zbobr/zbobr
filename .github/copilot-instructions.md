❯ - take an open issue in the https://github.com/milyin/copilot repository with milestone PLANNING in priority order
  - read the issue and comments
  - investgate issue, determine to which project(s) mentioned above the issue is related
  - make or update implementaion plan, ask questions, edit and comment the issue. Create subissues if needed. The up-to-date plan is always should be in the issue description.
  - set the issue to PENDING milestone

  - take an open issue with milestone READY in priority order
  - set the milestone to WORKING
  - use label "model:<name>" for model selection for agent. Use "GPT-5 Mini" if not specified
  - assign the issue to an agent (see agent instruction)

  Agent instuction:
  - if not done yet:
    - clone the target <repository> to "milyin"
    - create new PR in the milyin/<repository> in format fix<issue_n>/short_and_descriptive_name,
    - add link to task from pr
  - read the comments to PR
  - do implementing the issue until done or stuck
  - add resuts / questions as a comment to PR
  - set corresponding issue to PENDING state


Tools:
- .github/scripts/update_issue_with_plan.sh — helper script that appends a plan file to an issue body under an 'Implementation plan' header and sets the issue milestone to PENDING.
  Usage: .github/scripts/update_issue_with_plan.sh <repo> <issue_number> <plan_file>
  Example: .github/scripts/update_issue_with_plan.sh milyin/copilot 1 /path/to/plan.md
  Requirements: gh CLI and an authenticated session (run 'gh auth login').

