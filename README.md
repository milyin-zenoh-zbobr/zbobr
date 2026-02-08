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
   - Created per-domain via `automation/setup/setup.sh --domain-project`

3. **Fork Owner**
   - Where Workers create temporary forks during implementation
   - Configured in domain project's `.zbobr.env` file
   - Can be a user (e.g., `milyin`) or organization (e.g., `YoroolGui`)

## Quick Start

**Prerequisites:**
- GitHub Copilot CLI (`copilot`)
- GitHub CLI (`gh`) authenticated
- Permission to create repos (for domain projects)

**Setup a domain project:**
```bash
# Example: Set up Zenoh domain project
automation/setup/setup.sh \
  --domain-project YoroolGui/copilot-zenoh \
  --fork-owner YoroolGui
```

This will:
- Create the domain project repo (if needed)
- Set up labels and milestones
- Initialize template files (instructions.md, repositories.md)
- Create `.zbobr.env` with fork owner configuration

**Configure target repositories:**

Add a `.zbobr.env` file to each target repository:
```bash
# .zbobr.env - zbobr configuration
ZBOBR_DOMAIN_REPO=YoroolGui/copilot-zenoh
ZBOBR_FORK_OWNER=YoroolGui
ZBOBR_DEFAULT_MODEL=gpt-5-mini
```

**Launch the orchestrator:**
```bash
# Run Manager in a loop (checks every 60 seconds)
automation/scripts/manager_loop.sh --interval 60
```

## How It Works

### Workflow

1. **Create issue** in domain project with milestone `PLANNING` and reference a target repo
2. **Manager researches** the issue and creates an implementation plan → sets `PENDING`
3. **Human reviews** and sets milestone to `READY` (optionally add `copilot:<model>` label for AI model choice)
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
- `copilot:<model>` — Use GitHub Copilot with specified model (e.g., `copilot:gpt-5-mini`, `copilot:claude-opus-4.6`)
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
│   ├── setup/               # Orchestrator-level scripts (zbobr context)
│   │   └── setup.sh         # Initialize domain projects
│   ├── scripts/             # Domain-level scripts (use .zbobr.env)
│   │   ├── lib.sh           # Common functions
│   │   ├── clone_target.sh  # Clone and fork target repos
│   │   ├── manager_loop.sh  # Run Manager on schedule
│   │   ├── agent.sh         # Agent launcher
│   │   ├── worker.sh        # Worker spawner
│   │   └── update_issue_with_plan.sh  # Plan updater
├── templates/
│   ├── domain-instructions.md
│   └── domain-repositories.md
├── AGENTS.md               # Agent registry
├── REPOSITORIES.md         # Example/documentation (not used by orchestrator)
└── README.md               # This file

YoroolGui/copilot-zenoh/ (Domain Project - created by setup.sh)
├── .zbobr.env             # zbobr configuration (fork owner, default model)
├── instructions.md        # Domain-specific guidance
└── repositories.md        # Target repos (zenoh projects)
```

## Usage Examples

### Set up a new domain project

```bash
# Create domain project for Apache Kafka ecosystem
automation/setup/setup.sh \
  --domain-project myorg/copilot-kafka \
  --fork-owner myorg
```

### Run Manager in background

```bash
# Clone domain project and cd into it
gh repo clone YoroolGui/copilot-zenoh
cd copilot-zenoh

# Run manager loop (poll every 30 seconds)
/path/to/zbobr/automation/scripts/manager_loop.sh --interval 30 &
```

### Manually invoke agents

```bash
# From domain project directory:
cd copilot-zenoh

# Process with Manager
/path/to/zbobr/automation/scripts/agent.sh manager

# Implement specific issue
/path/to/zbobr/automation/scripts/agent.sh worker 42
```

### Dry-run setup

```bash
# See what would be created without making changes
automation/setup/setup.sh \
  --domain-project YoroolGui/copilot-zenoh \
  --fork-owner YoroolGui \
  --dry-run
```

## Configuration

### Domain Project Setup

When you run `setup.sh` with `--domain-project`, it automatically creates:
- `instructions.md` — Domain-specific guidance for agents (from template)
- `repositories.md` — List of target repositories (from template)

**Note:** Existing files are never overwritten. Customize after initial setup.

Edit `<domain-project>/repositories.md` to list which repos the domain manages:

```markdown
# Target Repositories

- https://github.com/zenoh/zenoh
- https://github.com/zenoh/rust-api
- https://github.com/zenoh/python-api
```

Edit `<domain-project>/instructions.md` for domain-specific guidance.

**See [REPOSITORIES.md](REPOSITORIES.md) for a complete real-world example (Zenoh project).**

### Domain Project Environment

Create a `.zbobr.env` file in your domain project to configure zbobr behavior:

```bash
# .zbobr.env - zbobr configuration for this domain project

# Required: This domain project's repository
ZBOBR_DOMAIN_REPO=YoroolGui/copilot-zenoh

# Required: User or organization where Worker agents create forks
ZBOBR_FORK_OWNER=YoroolGui

# Optional: Default AI model for issues without model: label
ZBOBR_DEFAULT_MODEL=gpt-5-mini
```

**Variables:**

| Variable | Required | Description |
|----------|----------|-------------|
| `ZBOBR_DOMAIN_REPO` | Yes | This domain project's repository |
| `ZBOBR_FORK_OWNER` | Yes | User or organization for creating forks |
| `ZBOBR_DEFAULT_MODEL` | No | Default AI model for issues |

Scripts must be run from the domain project directory and automatically load `.zbobr.env`:
```bash
cd copilot-zenoh  # domain project directory
/path/to/zbobr/automation/scripts/clone_target.sh zenoh/zenoh 123
```

### Available Models

The setup script automatically detects available models from the Copilot CLI. Common options:

- `gpt-5-mini` (default, free)
- `claude-haiku-4.5`
- `gpt-5`, `claude-sonnet-4.5`
- `claude-opus-4.6`

Use labels like `copilot:claude-opus-4.6` on issues to select a specific model.

## GitHub Authentication

Zbobr uses the GitHub API via a personal access token. It reads the token from the `GH_TOKEN` environment variable (or `GITHUB_TOKEN` as fallback).

**If you already have `gh` CLI authenticated** (i.e., `gh auth status` shows you're logged in), you can reuse that session — no separate token or login is needed:

```bash
# Export your existing gh session token for zbobr to use
export GH_TOKEN=$(gh auth token)
```

Add this to your shell profile (e.g., `~/.bashrc`, `~/.zshrc`) to make it persistent:

```bash
# In ~/.zshrc or ~/.bashrc
export GH_TOKEN=$(gh auth token)
```

**Alternative: use a personal access token directly.** If you prefer not to depend on `gh`, you can create a [GitHub Personal Access Token](https://github.com/settings/tokens) with `repo` scope and set it manually:

```bash
export GH_TOKEN=ghp_xxxxxxxxxxxxxxxxxxxx
```

**Token resolution order:**
1. `GH_TOKEN` environment variable (preferred — matches `gh` CLI convention)
2. `GITHUB_TOKEN` environment variable (fallback — matches GitHub Actions convention)

**Required token permissions:** The token needs `repo` scope (full access to repositories) to create forks, manage issues/labels/milestones, and push branches.

## Important Notes

- **All issue/PR management** happens in the domain project repository
- **Agents never modify** target repositories directly—they only create forks and PRs
- **Labels and milestones** are orchestrator-managed and universal
- **Domain projects are configurable** — each domain defines its own target repositories
- **Fork owners can be users or organizations** — no GitHub org creation required

## Documentation

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
