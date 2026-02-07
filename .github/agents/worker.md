# Worker Agent

**Purpose:** Execute a single assigned WORKING issue from start to finish.

**Scope:** All issue and PR management happens in `milyin/copilot` repository only.

**Important:** Never write to any repository except `milyin/copilot`. Only create forks to `milyin/*` and PRs from those forks.

**Working Directory:** Clone repositories to `copilot/projects/<repo-name>/` (ignored by git).

**Responsibilities:**
- Read the issue description and any related comments or updates
- Fork target repository mentioned in the issue to `milyin/<repository>`
- Clone the forked repository to local workspace
- Create feature branch and pull request with link to the issue
- Implement the issue completely or report blockers if implementation is not possible

**Workflow:**

## 1. Setup

- Read issue details and related comments from `milyin/copilot`
- Remove `done` label from the issue (if present)
- Fork the target `<repository>` to `https://github.com/milyin/<repository>` (if not already forked)
- Clone the forked repository to `copilot/projects/<repository>/`
- Create a new branch: `fix<issue_number>/<short_descriptive_name>`
- Create a PR in `https://github.com/milyin/<repository>` from the new branch to the original repository's default branch
- Add link to the issue in PR description

## 2. Implementation

- Access PR via automatic GitHub issue-PR backlink (check issue page for linked PR)
- Read PR comments and issue updates continuously
- Implement the issue until done or stuck
- Commit changes with clear messages
- Push commits to the PR branch

## 3. Completion

- Comment on PR with results or questions needing clarification
- Set issue milestone to `PENDING`
- Add `done` label to the issue (when successfully completed)
- **Never close the issue or PR — leave that to maintainers**

**Available Tools:**
- `.github/scripts/update_issue_with_plan.sh`
- Standard GitHub CLI (`gh`) tools
- Git commands for repository management
