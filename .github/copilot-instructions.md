## Invoking Agents

### Manager Agent
Coordinates issue workflow and spawns Workers.

**From CLI:**
```bash
.github/scripts/agent.sh manager
```

**From GitHub Copilot Chat:**
```
@manager help with issue planning
```

**Definition:** [.github/agents/manager.md](.github/agents/manager.md)

---

### Worker Agent
Executes a single WORKING issue end-to-end.

**From CLI:**
```bash
.github/scripts/agent.sh worker <issue_number> <repo> [model]
```

**From GitHub Copilot Chat:**
```
@worker fix issue #123 in milyin/copilot
```

**Definition:** [.github/agents/worker.md](.github/agents/worker.md)

---

## Tools

Both Manager and Worker agents can use these tools:

- **update_issue_with_plan.sh** — Appends a plan file to an issue body under an 'Implementation plan' header and sets the issue milestone to PENDING.
  - Usage: `.github/scripts/update_issue_with_plan.sh <repo> <issue_number> <plan_file>`
  - Example: `.github/scripts/update_issue_with_plan.sh milyin/copilot 1 /path/to/plan.md`
  - Requirements: `gh` CLI and authenticated session (`gh auth login`)

- **run_worker_script** — Spawns a Worker agent to handle a READY issue (used by Manager).
  - Usage: `gh copilot run-worker --issue <issue_number> --repo <repo> --model <model_name>`
  - Purpose: Initiates a Worker agent to implement the issue
  - Requirements: `gh` CLI and authenticated session

