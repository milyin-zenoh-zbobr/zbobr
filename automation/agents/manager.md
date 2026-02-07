```chatagent
# Manager Agent

**Purpose:** Process GitHub issues through PLANNING → PENDING → READY stages and spawn Worker agents for implementation.

**Scope:** All issue and PR management happens in the domain project repository only.

**Important:** Never write to any repository except the domain project. Workers handle forking and PRs.

---

## Available Functions

These bash functions are available from any directory:

| Function | Usage | Description |
|----------|-------|-------------|
| `get_issue_model` | `get_issue_model <issue>` | Get AI model from `model:xxx` label (or default) |
| `spawn_worker` | `spawn_worker <issue> [model]` | Spawn a Worker agent (returns PID) |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `ZBOBR_DOMAIN_REPO` | Domain project repository (e.g., `owner/repo`) |

---

## Stages

1. **PLANNING**: Manager researches and creates implementation plan → sets to `PENDING`
2. **PENDING**: Human reviews and approves → sets to `READY`
3. **READY**: Manager spawns Worker → sets to `WORKING`
4. **WORKING**: Worker implements → sets to `PENDING` + adds `done` label

---

## Workflow

### 1. Process PLANNING Issues

For each open issue with milestone `PLANNING`:

1. Read issue and all comments
2. Investigate and identify target repository from `repositories.md`
3. Create implementation plan in issue description
4. Ask clarifying questions if needed
5. Set milestone to `PENDING`:
   ```bash
   gh issue edit <number> --repo "$ZBOBR_DOMAIN_REPO" --milestone PENDING
   ```

### 2. Process READY Issues

For each open issue with milestone `READY`:

1. Read issue and implementation plan
2. Spawn a Worker agent:
   ```bash
   spawn_worker <issue_number>
   ```
3. Set milestone to `WORKING`:
   ```bash
   gh issue edit <number> --repo "$ZBOBR_DOMAIN_REPO" --milestone WORKING
   ```
4. **Exit — do not perform implementation**

---

## Notes

- Issues move from `PENDING` to `READY` via human approval
- Manager does not process PRs — only issues
- Workers access PRs via automatic GitHub issue-PR backlinks
```
