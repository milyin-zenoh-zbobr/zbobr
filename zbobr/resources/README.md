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
| `tool:<name>` | Specifies which tool to use (e.g., `tool:copilot`, `tool:claude`) |
| `model:<name>` | Specifies which AI model to use (e.g., `model:claude-3-opus`, `model:gpt-4o`) |
| `done` | Implementation is complete, PR has been created |

If no `tool:` or `model:` labels are set, the defaults from configuration are used.

## Creating Issues

1. Create a new issue describing what needs to be done
2. Set the milestone to **PLANNING** to start automated processing
3. Optionally add `tool:<name>` and `model:<name>` labels to customize the agent
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

## Prompts

The `prompts/` directory contains markdown files that provide context, guidelines, and technical workflow instructions to AI agents. These files are automatically included when agents process issues.

### Prompt Files

| File | Used By | Purpose |
|------|---------|---------|
| `common.md` | Planner & Worker | Shared context about project architecture, conventions, and domain knowledge |
| `repositories.md` | Planner | Lists target repositories and repository-specific notes |
| `planner.md` | Planner | Domain-specific prompts for the planning phase |
| `worker.md` | Worker | Domain-specific prompts for the implementation phase |
| `planner-workflow.md` | Planner | Technical workflow and MCP API documentation |
| `worker-workflow.md` | Worker | Technical workflow and MCP API documentation |

### Default Configuration

- **Planner agents** receive: `common.md`, `repositories.md`, and `planner.md`
- **Worker agents** receive: `common.md` and `worker.md`

### Customizing Prompts

You can customize which files are used by editing `.zbobr.env`:

```bash
# Semicolon-separated list of prompt files
ZBOBR_PLANNER_PROMPTS=prompts/common.md;prompts/repositories.md;prompts/planner.md
ZBOBR_WORKER_PROMPTS=prompts/common.md;prompts/worker.md
```

### Editing Prompts

1. **Edit existing files**: Modify the files in `prompts/` to add project-specific context
2. **Add new files**: Create additional markdown files and reference them in `.zbobr.env`
3. **Remove files**: Remove file paths from the environment variables

### What to Include

**In common.md**:
- Architecture patterns (microservices, monolith, etc.)
- Technology stack (frameworks, databases, tools)
- Coding conventions and style guides
- Domain concepts and terminology

**In repositories.md**:
- List of target repositories
- Repository-specific notes and conventions
- Build and test commands
- Deployment information

**In planner.md**:
- How to structure implementation plans
- What level of detail to include
- Specific investigation approaches
- Planning output format preferences

**In worker.md**:
- Code quality expectations
- Testing requirements
- Commit message format
- PR description template
- When to ask for guidance


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
