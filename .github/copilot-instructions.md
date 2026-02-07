## Invoking Agents

Use the `copilot` CLI for named agents. The `/agent` command is available only in Copilot CLI interactive mode.

### Manager Agent
Coordinates issue workflow and spawns Workers.

**From Copilot CLI:**
```bash
copilot --agent manager -i "Process issues using the manager workflow."
```

**Interactive `/agent` selection:**
```bash
copilot
# then run /agent
```

**From GitHub Copilot Chat:**
```
@manager help with issue planning
```

**Definition:** [automation/agents/manager.md](automation/agents/manager.md)

---

### Worker Agent
Executes a single WORKING issue end-to-end.

**From Copilot CLI:**
```bash
copilot --agent worker --model gpt-5-mini -i "Fix issue #123 in milyin/copilot."
```

**From GitHub Copilot Chat:**
```
@worker fix issue #123 in milyin/copilot
```

**Definition:** [automation/agents/worker.md](automation/agents/worker.md)

**Agent Registry:** [AGENTS.md](AGENTS.md)

---

## Tools

Both Manager and Worker agents can use these tools:

- **update_issue_with_plan.sh** — Appends a plan file to an issue body under an 'Implementation plan' header and sets the issue milestone to PENDING.
  - Usage: `automation/scripts/update_issue_with_plan.sh <repo> <issue_number> <plan_file>`
  - Example: `automation/scripts/update_issue_with_plan.sh milyin/copilot 1 /path/to/plan.md`
  - Requirements: `gh` CLI and authenticated session (`gh auth login`)

- **worker.sh** — Spawns a Worker agent to handle a READY issue (used by Manager).
  - Usage: `automation/scripts/worker.sh --issue <issue_number> --model <model_name>`
  - Purpose: Initiates a Worker agent to implement the issue
  - Requirements: `gh` CLI and authenticated session

- **lib.sh** — Common library functions for repository operations and label/milestone management.
  - Usage: `source automation/scripts/lib.sh`
  - Default REPO: `milyin/copilot` (can be overridden: `REPO="other/repo" source lib.sh`)
  - Functions: `reconcile_lists`, `get_labels`, `get_milestones`, `extract_model_from_labels`, etc.
  - Used by other scripts for consistent repository interactions

