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

### zbobr.toml

The `zbobr.toml` file is the primary configuration for this domain project:

```toml
# Domain project repository ("owner/repo")
domain_repo = "your-org/domain-project"

# GitHub user or org where target repos are forked
fork_owner = "your-username"

# Default AI model (e.g. "gpt-5-mini", "claude-sonnet-4.5")
# default_model = "gpt-5-mini"

# Workspace directory for task work dirs
# workspace = "./workspace"

# CLI tool: "copilot", "claude", or "stub"
# cli_tool = "copilot"

# Work branch prefix
# work_branch_prefix = "zbobr_fix"

[prompts]
# Base directory for additional prompt files (appended after built-in instructions)
# path = "./prompts"
# Additional context files for planner
# planner = ["planner.md", "common.md"]
# Additional context files for worker
# worker = ["worker.md", "common.md"]
```

Configuration priority: CLI args > environment variables > `zbobr.toml` > defaults.

**GitHub Token**: Set `GH_TOKEN` in your environment:
```bash
export GH_TOKEN=$(gh auth token)
```
Or create a token at https://github.com/settings/tokens (needs `repo` scope).

## Prompts

Core agent instructions (workflow, MCP tool usage, access model) are built into zbobr and always included automatically. The `prompts/` directory contains additional context files that are appended after the built-in instructions.

### Prompt Files

| File | Used By | Purpose |
|------|---------|---------|
| `common.md` | Planner & Worker | Shared context about project architecture, conventions, and domain knowledge |
| `planner.md` | Planner | Additional planner-specific context (empty by default) |
| `worker.md` | Worker | Additional worker-specific context (empty by default) |

### Default Configuration

- **Planner agents** receive: built-in instructions + `planner.md` + `common.md` + API docs
- **Worker agents** receive: built-in instructions + `worker.md` + `common.md` + API docs

### Customizing Prompts

You can customize which additional context files are used in `zbobr.toml`:

```toml
[prompts]
path = "./prompts"
planner = ["planner.md", "common.md"]
worker = ["worker.md", "common.md"]
```

### Editing Prompts

1. **Edit existing files**: Modify the files in `prompts/` to add project-specific context
2. **Add new files**: Create additional markdown files and reference them in `zbobr.toml`
3. **Remove files**: Remove file paths from the prompts configuration

### What to Include

**In common.md**:
- Architecture patterns (microservices, monolith, etc.)
- Technology stack (frameworks, databases, tools)
- Coding conventions and style guides
- Domain concepts and terminology
- List of target repositories and repository-specific notes
- Build and test commands
- Deployment information

**In planner.md** (additional context beyond built-in instructions):
- Project-specific investigation approaches
- Custom planning standards or templates

**In worker.md** (additional context beyond built-in instructions):
- Project-specific code quality requirements
- Custom commit message or PR description formats


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
