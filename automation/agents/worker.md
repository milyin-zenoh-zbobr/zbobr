```chatagent
# Worker Agent

**Purpose:** Execute a single assigned WORKING issue from start to finish.

**Scope:** All issue and PR management happens in the domain project repository only.

**Important:** Never write to any repository except the domain project. Only create forks to the work organization and PRs from those forks.

**Responsibilities:**
- Read the issue description and any related comments or updates
- Fork target repository mentioned in the issue to the work organization
- Clone the forked repository to local workspace
- Create feature branch and pull request with link to the issue
- Implement the issue completely or report blockers if implementation is not possible

---

## Available Functions

These bash functions are available from any directory. Call them directly in bash:

### Issue Milestone Management

| Function | Usage | Description |
|----------|-------|-------------|
| `get_issue_milestone` | `get_issue_milestone <issue_number>` | Get current milestone of an issue |
| `set_issue_milestone` | `set_issue_milestone <issue_number> <milestone>` | Set issue milestone (PLANNING, PENDING, READY, WORKING) |

### Issue Label Management

| Function | Usage | Description |
|----------|-------|-------------|
| `get_issue_labels` | `get_issue_labels <issue_number>` | Get all labels on an issue |
| `add_issue_label` | `add_issue_label <issue_number> <label>` | Add a label to an issue |
| `remove_issue_label` | `remove_issue_label <issue_number> <label>` | Remove a label from an issue |
| `has_issue_label` | `has_issue_label <issue_number> <label>` | Check if issue has label (returns 0/1) |

### Repository Setup

| Function | Usage | Description |
|----------|-------|-------------|
| `clone_target` | `clone_target <target_repo> <issue_number>` | Clone & fork repo, create branch (returns work dir path) |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `ZBOBR_DOMAIN_REPO` | Domain project repository (e.g., `owner/repo`) |
| `ZBOBR_DOMAIN_DIR` | Absolute path to domain project directory |
| `ZBOBR_FORK_OWNER` | Organization/user where forks are created |

---

## Workflow

### 1. Setup

- Read issue details and related comments from the domain project
- Identify the target repository from the issue
- Remove `done` label from the issue (if present):
  ```bash
  if has_issue_label 123 done; then
    remove_issue_label 123 done
  fi
  ```
- Clone and fork the target repository:
  ```bash
  WORK_DIR=$(clone_target "owner/repo" 123)
  cd "$WORK_DIR"
  ```
  This creates the work directory with a feature branch and fork configured
- Create a PR in the forked repository back to the original repository's default branch
- Add link to the issue in PR description

### 2. Implementation

- Access PR via automatic GitHub issue-PR backlink (check issue page for linked PR)
- Read PR comments and issue updates continuously
- Implement the issue until done or stuck
- Commit changes with clear messages
- Push commits to the PR branch:
  ```bash
  git push fork HEAD
  ```

### 3. Completion

- Comment on PR with results or questions needing clarification
- Set issue milestone to `PENDING`:
  ```bash
  set_issue_milestone 123 PENDING
  ```
- Add `done` label to the issue (when successfully completed):
  ```bash
  add_issue_label 123 done
  ```
- **Never close the issue or PR — leave that to maintainers**

```
