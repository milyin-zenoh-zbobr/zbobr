# Domain Project

This repository is managed by [**zbobr**](https://github.com/milyin/zbobr) -- an AI-powered issue orchestrator that uses Claude Code agents to automate software development workflows.

> **Source**: [milyin/zbobr](https://github.com/milyin/zbobr)
> **Documentation**: See the main repository for installation, architecture, and advanced usage.

## Overview

Zbobr transforms GitHub issues into automated development tasks. AI agents (planner and worker) process issues through defined stages, while humans review and approve at key checkpoints. All agent interactions happen through MCP (Model Context Protocol) tools, keeping the workflow technology-agnostic.

## Workflow Stages

Issues progress through milestones that represent their current stage:

| Milestone | Description | Who |
|-----------|-------------|-----|
| **PLANNING** | Agent investigates the issue and creates an implementation plan | Planner agent |
| **PENDING** | Plan or implementation is complete, awaiting human review | Human reviewer |
| **READY** | Plan approved, ready for implementation | Human sets this |
| **WORKING** | Agent is actively implementing the issue | Worker agent |

### Stage Flow

```
New issue ──► PLANNING ──► PENDING ──► READY ──► WORKING ──► PENDING (done)
                              │                                  │
                              ▼                                  ▼
                        Human reviews                     Human reviews
                        and approves                      and closes
```

## Labels

| Label | Description |
|-------|-------------|
| `copilot:<model>` | Specifies which AI model to use (e.g., `copilot:claude-opus-4.6`) |
| `done` | Implementation is complete, PR has been created |

If no `copilot:` label is set, the default model from configuration is used.

## Creating Issues

1. Create a new issue describing what needs to be done
2. Set the milestone to **PLANNING** to start automated processing
3. Optionally add a `copilot:<model>` label to choose a specific model
4. The planner agent will investigate and write an implementation plan
5. Review the plan when it reaches **PENDING**
6. Set milestone to **READY** to approve implementation
7. The worker agent will implement, create a PR, and mark as `done`

## Configuration

### Environment Variables

The `.zbobr.env` file contains configuration for this domain project:

```bash
# Required: GitHub repository whose issues the orchestrator processes (owner/repo)
ZBOBR_DOMAIN_REPO=your-org/domain-project

# Required: GitHub user or organization where target repos are forked
ZBOBR_FORK_OWNER=your-username

# Optional: Default AI model when no copilot:<model> label is set
ZBOBR_DEFAULT_MODEL=claude-sonnet-4.5

# Optional: Directory for agent workspaces (cloned repos, temp files)
ZBOBR_WORKSPACE=./workspace

# Optional: Custom prompt files for planner and worker agents
ZBOBR_PLANNER_PROMPT=/path/to/custom-planner.md
ZBOBR_WORKER_PROMPT=/path/to/custom-worker.md
```

**GitHub Token**: Set `GH_TOKEN` in your environment (not in `.zbobr.env`):
```bash
export GH_TOKEN=$(gh auth token)
```
Or create a token at https://github.com/settings/tokens (needs `repo` scope).

### Launcher Scripts

- `run.sh` (Unix/Linux/macOS): Bash script to start the zbobr orchestrator
- `run.cmd` (Windows): Batch script for Windows environments

These scripts load `.zbobr.env` and launch the zbobr daemon with proper configuration.

## Target Repositories

The `repositories.md` file lists which repositories this domain project manages. When issues are assigned to agents, they fork these repositories to create feature branches and pull requests.

**To customize**:
1. Edit `repositories.md` in this repository
2. Add GitHub repository URLs (one per line or as a bulleted list)
3. Format: `https://github.com/owner/repo-name`

Example `repositories.md`:
```markdown
# Target Repositories

- https://github.com/myorg/backend-api
- https://github.com/myorg/frontend-app
- https://github.com/myorg/shared-library
```

Agents will:
1. Fork listed repositories to `ZBOBR_FORK_OWNER`
2. Create feature branches in the forks
3. Implement changes and push commits
4. Open pull requests back to the original repositories

## Customizing Agent Prompts

Agent behavior is controlled by markdown prompt files. By default, zbobr uses built-in prompts, but you can customize them:

### Default Prompts Location

If you cloned the zbobr repository, default prompts are in:
- `automation/agents/planner.md` - Planner agent instructions
- `automation/agents/worker.md` - Worker agent instructions

### Using Custom Prompts

1. **Copy and modify** the default prompts:
   ```bash
   mkdir -p custom-prompts
   cp /path/to/zbobr/automation/agents/planner.md custom-prompts/my-planner.md
   cp /path/to/zbobr/automation/agents/worker.md custom-prompts/my-worker.md
   ```

2. **Configure zbobr** to use your custom prompts in `.zbobr.env`:
   ```bash
   ZBOBR_PLANNER_PROMPT=./custom-prompts/my-planner.md
   ZBOBR_WORKER_PROMPT=./custom-prompts/my-worker.md
   ```

3. **Restart zbobr** to load the new prompts.

### Prompt Customization Ideas

- **Domain-specific instructions**: Add context about your project's architecture, coding standards, or conventions
- **Tool preferences**: Specify preferred libraries, frameworks, or patterns
- **Quality gates**: Add instructions for testing, documentation, or review requirements
- **Output format**: Customize how agents document their work or structure commits

### Testing Custom Prompts

Use zbobr CLI commands to test agents with custom prompts:
```bash
# Test planner with specific issue
zbobr plan --task 123 --planner-prompt ./custom-prompts/my-planner.md

# Test worker with specific issue
zbobr work --task 456 --worker-prompt ./custom-prompts/my-worker.md
```

## Workflow Deep Dive

### How Issues Progress

1. **Create Issue**: Developer creates a GitHub issue in this domain repository
2. **Set PLANNING Milestone**: Triggers the planner agent to investigate
3. **Planner Investigates**: Agent reads code, analyzes requirements, drafts plan
4. **Move to PENDING**: Planner completes plan and waits for human review
5. **Human Reviews Plan**: Developer reviews, requests changes, or approves
6. **Set READY Milestone**: Approval triggers worker agent
7. **Worker Implements**: Agent forks repos, writes code, creates commits
8. **Move to PENDING + `done` Label**: Worker creates PR and waits for review
9. **Human Reviews PR**: Developer reviews code, merges, and closes issue

### Agent Isolation

- Each agent (planner/worker) runs in an isolated session with its own MCP tools
- Agents never directly access GitHub API - all operations go through zbobr's MCP interface
- This isolation allows swapping agent implementations without changing zbobr core

### Human Checkpoints

Human approval is required at:
- **After Planning**: Review the implementation plan before work begins
- **After Implementation**: Review the actual code changes in the pull request

This ensures AI assistance remains under human control and oversight.
