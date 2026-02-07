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

**Important:** All scripts in `automation/scripts/` must be run from the domain project directory (which contains `.zbobr.env`).

- **update_issue_with_plan.sh** — Appends a plan file to an issue body and sets milestone to PENDING.
  - Usage: `automation/scripts/update_issue_with_plan.sh <issue_number> <plan_file>`
  - Example: `automation/scripts/update_issue_with_plan.sh 1 /path/to/plan.md`
  - Requirements: Run from domain project directory, `gh` CLI authenticated

- **worker.sh** — Spawns a Worker agent to handle a READY issue.
  - Usage: `automation/scripts/worker.sh --issue <issue_number> [--model <model_name>]`
  - Example: `automation/scripts/worker.sh --issue 42`
  - Model defaults to `$ZBOBR_DEFAULT_MODEL` or `gpt-5-mini`
  - Requirements: Run from domain project directory

- **clone_target.sh** — Clones and forks a target repository for issue implementation.
  - Usage: `automation/scripts/clone_target.sh <target_repo> <issue_number>`
  - Example: `automation/scripts/clone_target.sh zenoh/zenoh 123`
  - Uses `$ZBOBR_FORK_OWNER` from `.zbobr.env`
  - Requirements: Run from domain project directory

- **agent.sh** — Agent CLI wrapper for Manager or Worker.
  - Usage: `automation/scripts/agent.sh manager [prompt]`
  - Usage: `automation/scripts/agent.sh worker <issue_number> [model]`
  - Requirements: Run from domain project directory

- **manager_loop.sh** — Runs Manager agent in a loop.
  - Usage: `automation/scripts/manager_loop.sh [--interval seconds]`
  - Requirements: Run from domain project directory

- **lib.sh** — Common library functions. Loads `.zbobr.env` automatically.
  - Functions: `reconcile_lists`, `get_labels`, `get_milestones`, `extract_model_from_labels`, `get_milestone_number`, `get_issue_milestone`, `set_issue_milestone`, `add_issue_label`, `remove_issue_label`, `has_issue_label`, etc.
