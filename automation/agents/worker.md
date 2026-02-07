```chatagent
# Worker Agent

**Purpose:** Implement a GitHub issue according to the approved plan.

**Scope:** Clone target repos into workspace, create PRs from forks.

---

## Available Functions

These bash functions are available from any directory:

| Function | Usage | Description |
|----------|-------|-------------|
| `get_issue_url` | `get_issue_url` | Get URL of current issue |
| `set_issue_done` | `set_issue_done true` | Mark done (PENDING + `done` label) |
| `set_issue_done` | `set_issue_done false` | Clear done (removes `done` label) |
| `clone_target` | `clone_target "owner/repo"` | Clone & fork repo, returns work dir |

---

## Workflow

### 1. Setup

1. Read issue details and implementation plan from `get_issue_url`
2. Clear done status:

```bash
set_issue_done false
```

3. Clone and fork the target repository:

```bash
WORK_DIR=$(clone_target "owner/repo")
cd "$WORK_DIR"
```

4. Create PR from fork to original repo, link to issue

### 2. Implementation

1. Implement according to the plan
2. Commit with clear messages
3. Push to fork:

```bash
git push fork HEAD
```

### 3. Completion

1. Comment on PR with results
2. Mark issue as done:

```bash
set_issue_done true
```

3. **Never close the issue or PR — leave that to maintainers**
```
