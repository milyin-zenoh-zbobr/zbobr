# Copilot Orchestrator

Universal AI-powered issue management automation using GitHub Copilot agents for coordinating and implementing fixes across multiple projects.

## Overview

This repository contains the **Copilot Orchestrator** — a reusable automation system that:

- **Manager Agent**: Processes issues through stages (PENDING → PLANNING_READY → PLANNING → WORKING_READY → WORKING), creates implementation plans, and spawns Worker agents
- **Worker Agent**: Implements individual issues by forking repos, creating PRs, and executing the work

The orchestrator is domain-agnostic and can manage any set of repositories through **Domain Projects**.

### Concepts

1. **Copilot Orchestrator** (this repo)
   - Universal automation system that processes issues through stages
   - Contains planner/worker agents and the `zbobr` CLI binary

2. **Domain Project** (`--domain-repo`)
   - A GitHub repository whose issues the orchestrator manages
   - Example: `YoroolGui/copilot-zenoh`
   - Contains: target repository list, project-specific guidance, and `zbobr.toml` config
   - Created via `zbobr setup --domain-repo owner/repo --fork-owner owner`

3. **Fork Owner** (`--fork-owner`)
   - The GitHub user or organization where target repos are forked for implementation
   - Worker agents fork repos under this account, create feature branches, and open PRs back to the original
   - Can be a personal account (e.g., `milyin`) or an organization (e.g., `YoroolGui`)
   - Configured in domain project's `zbobr.toml` file or via `--fork-owner` CLI flag

## Installation

Install zbobr using Cargo:

```bash
# Install from local source (if you've cloned the repo)
cargo install --path zbobr

# Or install directly from GitHub
cargo install --git https://github.com/milyin/zbobr.git zbobr
```

Verify installation:
```bash
zbobr --help
```

The `zbobr` binary will be installed to `~/.cargo/bin/` (make sure this is in your `PATH`).

**Add to PATH (if needed):**
```bash
# Add to ~/.bashrc or ~/.zshrc
export PATH="$HOME/.cargo/bin:$PATH"
```

## Quick Start

**Prerequisites:**
- GitHub Copilot CLI (`copilot`)
- GitHub CLI (`gh`) authenticated
- Permission to create repos (for domain projects)
- Rust and Cargo (for installation)

**Setup a domain project:**
```bash
# Ensure your GitHub token is available
export GH_TOKEN=$(gh auth token)

# Example: Set up Zenoh domain project
zbobr setup --domain-repo YoroolGui/copilot-zenoh --fork-owner YoroolGui
```

This will:
- Create the domain project repo on GitHub (if needed)
- Set up labels and milestones
- Initialize template files (README.md, prompt files)
- Create `zbobr.toml` with configuration

Use `--dry-run` to preview local files without pushing to GitHub.

**Launch the orchestrator:**
```bash
# Run the manager loop (polls every 60 seconds)
zbobr loop --domain-repo YoroolGui/copilot-zenoh --fork-owner YoroolGui
```

**Using zbobr.toml (from domain project):**

After setting up a domain project, you can clone it and run `zbobr` directly — it automatically reads `zbobr.toml` from the current directory:

```bash
# Clone the domain project
git clone https://github.com/YoroolGui/copilot-zenoh.git
cd copilot-zenoh

# Run zbobr commands (automatically uses zbobr.toml config)
zbobr loop                # Start the orchestrator loop
zbobr plan 42            # Run planner on issue #42
zbobr work 42            # Run worker on issue #42
```

This is the recommended approach for domain-specific workflows, as it eliminates the need to manually specify `--domain-repo` and `--fork-owner` flags.

## How It Works

### Workflow

1. **Create issue** in domain project with milestone `PLANNING_READY` and reference a target repo
2. **Manager researches** the issue (transitioning to `PLANNING` lock state) and creates an implementation plan → sets `PENDING`
3. **Human reviews** and sets milestone to `WORKING_READY` (optionally add `copilot:<model>` label for AI model choice)
4. **Manager spawns Worker** (transitioning to `WORKING` lock state)  
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
**Milestones:**
- `PENDING` → Issue is under user's control, bot ignores it
- `PLANNING_READY` → Issue must be taken by planner agent, any matching bot can take it
- `PLANNING` → Issue is in planning, other bots ignore it
- `WORKING_READY` → Issue must be taken by worker agent, any matching bot can take it
- `WORKING` → Issue is in work, other bots ignore it

## Architecture

```
milyin/copilot/ (Orchestrator)
├── automation/
│   ├── agents/
│   │   ├── manager.md       # Manager agent instructions
│   │   └── worker.md        # Worker agent instructions
│   ├── setup/               # Orchestrator-level scripts (zbobr context)
│   │   └── setup.sh         # Initialize domain projects
│   ├── scripts/             # Domain-level scripts
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
├── zbobr.toml             # zbobr configuration (fork owner, default model)
├── prompts/
│   ├── common.md          # Shared context and domain knowledge
│   ├── planner.md         # Planner agent instructions
│   └── worker.md          # Worker agent instructions
└── README.md              # Setup instructions
```

## Usage Examples

### Set up a new domain project

```bash
# Create domain project for Apache Kafka ecosystem
zbobr setup --domain-repo myorg/copilot-kafka --fork-owner myorg

# Preview without pushing to GitHub
zbobr setup --domain-repo myorg/copilot-kafka --fork-owner myorg --dry-run
```

### Run the manager loop

```bash
# Poll for issues every 30 seconds, clean up every 10 minutes
zbobr loop --domain-repo YoroolGui/copilot-zenoh --fork-owner YoroolGui \
  --interval 30 --cleanup-interval 600
```

### Manually run agents for a specific issue

```bash
# Run planner on issue #42 (creates implementation plan)
zbobr plan 42 --domain-repo YoroolGui/copilot-zenoh --fork-owner YoroolGui

# Run worker on issue #42 (implements the plan, creates PR)
zbobr work 42 --domain-repo YoroolGui/copilot-zenoh --fork-owner YoroolGui
```

## Configuration

### Domain Project Setup

`zbobr setup` creates the following files in the domain project:
- `README.md` — Overview of the domain project workflow
- `prompts/common.md` — Shared context and target repositories
- `prompts/planner.md` — Planner agent instructions
- `prompts/worker.md` — Worker agent instructions
- `zbobr.toml` — Configuration (domain repo, fork owner, etc.)

**Note:** Existing files on GitHub are never overwritten. Customize after initial setup.

Edit `prompts/common.md` to list which repos the domain manages:

```markdown
# Target Repositories

- https://github.com/zenoh/zenoh
- https://github.com/zenoh/rust-api
- https://github.com/zenoh/python-api
```

### Configuration

All settings can be provided via CLI flags, environment variables, or `zbobr.toml`:

| CLI Flag | Env Variable | Required | Description |
|----------|-------------|----------|-------------|
| `--domain-repo` | `ZBOBR_DOMAIN_REPO` | Yes | GitHub repo whose issues the orchestrator processes (`owner/repo`) |
| `--fork-owner` | `ZBOBR_FORK_OWNER` | Yes | GitHub user or org where target repos are forked for implementation |
| | `ZBOBR_DEFAULT_MODEL` | No | Default AI model when no `copilot:<model>` label is set |
| | `ZBOBR_WORKSPACE` | No | Directory for agent workspaces (default: `./workspace`) |
| `--planner-prompt` | `ZBOBR_PLANNER_PROMPT` | No | Custom planner agent prompt file |
| `--worker-prompt` | `ZBOBR_WORKER_PROMPT` | No | Custom worker agent prompt file |

CLI flags take priority over environment variables.

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

- [Domain Project Guide](DOMAIN_PROJECT.md) — How domain projects work (stages, labels, prompts, configuration)
- [Sample Configuration](zbobr.toml.sample) — Example `zbobr.toml` with all options
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
