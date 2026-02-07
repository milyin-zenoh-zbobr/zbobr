# Copilot Orchestrator

Universal AI-powered issue management automation using GitHub Copilot agents for coordinating and implementing fixes across multiple projects.

## Overview

This repository contains the **Copilot Orchestrator** — a reusable automation system that:

- **Manager Agent**: Processes issues through stages (PLANNING → PENDING → READY → WORKING), creates implementation plans, and spawns Worker agents
- **Worker Agent**: Implements individual issues by forking repos, creating PRs, and executing the work

The orchestrator is domain-agnostic and can manage any set of repositories through **Domain Projects**.

### Concepts

1. **Copilot Orchestrator** (this repo)
   - Universal automation system
   - Contains Manager/Worker agents and common scripts
   - Location: `milyin/copilot`

2. **Domain Project** 
   - Task/project-specific configuration repo
   - Example: `YoroolGui/copilot-zenoh`
   - Contains: Target repository list and project-specific guidance
   - Created per-domain via `automation/scripts/setup.sh --domain-project`

3. **Fork Owner**
   - Where Workers create temporary forks during implementation
   - Can be a user (e.g., `milyin`) or organization (e.g., `YoroolGui`)

## Quick Start

**Prerequisites:**
- GitHub Copilot CLI (`copilot`)
- GitHub CLI (`gh`) authenticated
- Permission to create repos (for domain projects)

**Setup a domain project:**
```bash
# Example: Set up Zenoh domain project
automation/scripts/setup.sh \
  --domain-project YoroolGui/copilot-zenoh \
  --fork-owner YoroolGui
```

This will:
- Create the domain project repo (if needed)
- Set up labels and milestones
- Initialize template files (instructions.md, repositories.md)

**Launch the orchestrator:**
```bash
# Run Manager in a loop (checks every 60 seconds)
automation/scripts/manager_loop.sh --interval 60
```

## How It Works

### Workflow

1. **Create issue** in domain project with milestone `PLANNING` and reference a target repo
2. **Manager researches** the issue and creates an implementation plan → sets `PENDING`
3. **Human reviews** and sets milestone to `READY` (optionally add `model:*` label for AI model choice)
4. **Manager spawns Worker** → sets `WORKING`  
5. **Worker implements** by:
   - Forking target repo to fork owner (e.g., `YoroolGui/*`)
   - Creating PR with link to issue
   - Implementing the fix
6. **Worker completes** by setting `PENDING` + adding `done` label
7. **Human reviews** PR and merges

### Labels & Milestones

These are orchestrator-owned and universal—same across all domain projects:

**Labels:**
- `model:*` — Specify which AI model to use (e.g., `model:gpt-5-mini`, `model:claude-opus-4.6`)
- `done` — Issue implementation is complete

**Milestones:**
- `PLANNING` → Manager researches and plans
- `PENDING` → Waiting for human review or implementation complete
- `READY` → Approved, ready for Worker to implement
- `WORKING` → Worker agent is actively implementing

## Architecture

```
milyin/copilot/ (Orchestrator)
├── automation/
│   ├── agents/
│   │   ├── manager.md       # Manager agent instructions
│   │   └── worker.md        # Worker agent instructions
│   ├── scripts/
│   │   ├── lib.sh           # Common functions
│   │   ├── setup.sh         # Initialize labs/milestones, create domain projects
│   │   ├── manager_loop.sh  # Run Manager on schedule
│   │   ├── agent.sh         # Agent launcher
│   │   ├── worker.sh        # Worker spawner
│   │   └── update_issue_with_plan.sh  # Plan updater
│   └── instructions.md      # How to invoke agents
├── templates/
│   ├── domain-instructions.md
│   └── domain-repositories.md
├── AGENTS.md               # Agent registry
└── README.md               # This file

YoroolGui/copilot-zenoh/ (Domain Project - created by setup.sh)
├── .copilot-config        # Configuration (fork_owner, etc.)
├── instructions.md        # Domain-specific guidance
└── repositories.md        # Target repos (zenoh projects)
```

## Usage Examples

### Set up a new domain project

```bash
# Create domain project for Apache Kafka ecosystem
automation/scripts/setup.sh \
  --domain-project myorg/copilot-kafka \
  --fork-owner myorg
```

### Run Manager in background

```bash
# Poll every 30 seconds (default is 60)
automation/scripts/manager_loop.sh --interval 30 &
```

### Manually invoke agents

```bash
# Process with Manager
copilot --agent manager -i "Process issues using the manager workflow."

# Implement specific issue
copilot --agent worker --model gpt-5 -i "Fix issue #42 in domain project."
```

### Dry-run setup

```bash
# See what would be created without making changes
automation/scripts/setup.sh \
  --domain-project YoroolGui/copilot-zenoh \
  --fork-owner YoroolGui \
  --dry-run
```

## Configuration

### Domain Project Setup

Edit `<domain-project>/repositories.md` to list which repos the domain manages:

```markdown
# Target Repositories

- https://github.com/zenoh/zenoh
- https://github.com/zenoh/rust-api
- https://github.com/zenoh/python-api
```

Edit `<domain-project>/instructions.md` for domain-specific guidance.

### Fork Owner Configuration

The fork owner (where Worker agents create temporary forks) is stored in the domain project's `.copilot-config` file. This is automatically created when running:

```bash
automation/scripts/setup.sh \
  --domain-project org/copilot-domain \
  --fork-owner org-or-user
```

Agents can read this configuration:
```bash
source automation/scripts/lib.sh
FORK_OWNER=$(get_fork_owner)
```

### Available Models

The setup script automatically detects available models from the Copilot CLI. Common options:

- `gpt-5-mini` (default, free)
- `claude-haiku-4.5`
- `gpt-5`, `claude-sonnet-4.5`
- `claude-opus-4.6`

Use labels like `model:claude-opus-4.6` on issues to select a specific model.

## Important Notes

- **All issue/PR management** happens in the domain project repository
- **Agents never modify** target repositories directly—they only create forks and PRs
- **Labels and milestones** are orchestrator-managed and universal
- **Domain projects are configurable** — each domain defines its own target repositories
- **Fork owners can be users or organizations** — no GitHub org creation required

## Documentation

- [Automation Instructions](automation/instructions.md) — How to invoke agents
- [Manager Agent Instructions](automation/agents/manager.md) — Manager workflow
- [Worker Agent Instructions](automation/agents/worker.md) — Worker workflow
- [Agent Registry](AGENTS.md) — Agent definitions for `/agent` command

## Development

Scripts are bash 3+ compatible (works on macOS default bash).

Validate scripts:
```bash
bash -n automation/scripts/*.sh
```

Make changes idempotent where possible (setup.sh can be run multiple times safely).
