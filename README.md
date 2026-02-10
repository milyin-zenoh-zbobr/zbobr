# Zbobr

AI-powered task orchestrator that manages GitHub issues through automated stages using pluggable AI tools (GitHub Copilot, Claude Code, or stubs for testing).

## Overview

This repository contains **zbobr** — a reusable automation system that:

- **Manager Agent**: Processes issues through stages (PENDING → GO_PLANNING → PLANNING → GO_WORKING → WORKING), creates implementation plans, and spawns Worker agents
- **Worker Agent**: Implements individual issues by forking repos, creating PRs, and executing the work

The orchestrator is domain-agnostic and can manage any set of repositories through **Domain Projects**.

### Concepts

1. **Zbobr** (this repo)
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
- Set up milestones (stages) and labels (`tool:*`, `model:*`, `done`)

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

1. **Create issue** in domain project with milestone `GO_PLANNING` and reference a target repo
2. **Manager researches** the issue (transitioning to `PLANNING` lock state) and creates an implementation plan → sets `PENDING`
3. **Human reviews** and sets milestone to `GO_WORKING` (optionally add `tool:<name>` and `model:<name>` labels)
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
- `tool:<name>` — Specifies which AI tool to use (e.g., `tool:copilot`, `tool:claude`, `tool:stub`)
- `model:<name>` — Specifies which AI model to use (e.g., `model:gpt-5-mini`, `model:claude-opus-4.6`)
- `done` — Issue implementation is complete

**Milestones:**
- `PENDING` → Issue is under user's control, bot ignores it
- `GO_PLANNING` → Issue must be taken by planner agent, any matching bot can take it
- `PLANNING` → Issue is in planning, other bots ignore it
- `GO_WORKING` → Issue must be taken by worker agent, any matching bot can take it
- `WORKING` → Issue is in work, other bots ignore it

## Architecture

```
zbobr/ (repo root)
├── zbobr/src/              # CLI binary (main.rs)
├── zbobr-lib/src/          # Library (backend, config, MCP, task model, etc.)
├── .github/agents/         # Agent instruction files (manager.md, worker.md)
├── .github/scripts/        # Legacy shell scripts
├── zbobr.toml.sample       # Sample configuration
├── DOMAIN_PROJECT.md       # Domain project guide
├── AGENTS.md               # Agent registry
└── README.md               # This file

YoroolGui/copilot-zenoh/ (Domain Project - created by zbobr setup)
├── zbobr.toml              # zbobr configuration (fork owner, default model)
├── prompts/
│   ├── common.md           # Shared context and domain knowledge
│   ├── planner.md          # Additional planner context
│   └── worker.md           # Additional worker context
└── README.md               # Setup instructions
```

## Usage Examples

### Set up a new domain project

```bash
# Create domain project for Apache Kafka ecosystem
zbobr setup --domain-repo myorg/copilot-kafka --fork-owner myorg

# Force-update existing labels
zbobr setup --domain-repo myorg/copilot-kafka --fork-owner myorg --force
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

All settings can be provided via CLI flags, environment variables, or `zbobr.toml`:

| CLI Flag | Env Variable | Description |
|----------|-------------|-------------|
| `--domain-repo` | `ZBOBR_DOMAIN_REPO` | GitHub repo whose issues the orchestrator processes (`owner/repo`) |
| `--fork-owner` | `ZBOBR_FORK_OWNER` | GitHub user or org where target repos are forked for implementation |
| `--workspace` | `ZBOBR_WORKSPACE` | Directory for agent workspaces (default: `./workspace`) |
| `--config` | `ZBOBR_CONFIG` | Path to TOML configuration file (default: `zbobr.toml` in cwd) |
| `--prompts-path` | `ZBOBR_PROMPTS_PATH` | Base directory for prompt files |
| `--planner-prompts` | `ZBOBR_PLANNER_PROMPTS` | Semicolon-separated list of prompt files for planner |
| `--worker-prompts` | `ZBOBR_WORKER_PROMPTS` | Semicolon-separated list of prompt files for worker |
| `--backend` | `ZBOBR_BACKEND` | Backend to use: `github` (default) or `stub` |
| `--cli-tool` | `ZBOBR_CLI_TOOL` | CLI tool to use: `copilot`, `claude`, or `stub` |
| `--admin-port` | `ZBOBR_ADMIN_PORT` | Port for the Admin MCP server (optional) |
| | `ZBOBR_DEFAULT_MODEL` | Default AI model when no `model:<name>` label is set |

Configuration priority: CLI flags > environment variables > `zbobr.toml` > defaults.

### Available Models

Zbobr supports 14+ models. Common options include:

- `gpt-5-mini` (default)
- `claude-sonnet-4.5`, `claude-opus-4.6`
- `gpt-5.2`, `gpt-5.2-codex`
- `gemini-3-pro-preview`

Use labels like `model:claude-opus-4.6` on issues to select a specific model, and `tool:copilot` or `tool:claude` to select the AI tool.

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

### GitHub Tokens for Agents and Copilot

Zbobr manages **three distinct GitHub tokens** with different access levels and purposes:

#### 1. Owner Token (`ZBOBR_OWNER_GH_TOKEN`)
- **Purpose**: Used by zbobr orchestrator for repository management (creating forks, managing issues, labels, milestones)
- **Access Level**: Write access to repositories
- **Resolution Order**:
  1. `ZBOBR_OWNER_GH_TOKEN` environment variable
  2. `GH_TOKEN` environment variable
  3. `GITHUB_TOKEN` environment variable
  4. `$(gh auth token)` (from gh CLI)
- **Config File**: `owner_github_token` in `zbobr.toml`

#### 2. Agent Token (`ZBOBR_AGENT_GH_TOKEN`) — **REQUIRED for restricted access**
- **Purpose**: Passed to agent processes (Copilot/Claude sessions) as `GH_TOKEN` and `GITHUB_TOKEN`
- **Access Level**: Read-only (should have minimal permissions)
- **Why**: Restricts what agents can do on GitHub—they can read repos but cannot push code or modify settings
- **Must Be Different From**: `ZBOBR_OWNER_GH_TOKEN` (security requirement—agents cannot have write access)
- **Resolution**: Explicitly set via environment variable or config file (no fallback)
- **Config File**: `agent_github_token` in `zbobr.toml`

#### 3. Copilot Token (`ZBOBR_COPILOT_GITHUB_TOKEN`) — **Required when restricting agent token**
- **Purpose**: Copilot CLI's own GitHub token (passed as `COPILOT_GITHUB_TOKEN` to agent sessions)
- **Access Level**: Copilot's own permissions (typically full access)
- **Why**: When you restrict `ZBOBR_AGENT_GH_TOKEN` for gh commands, Copilot itself needs its own full-access token to work properly
- **⚠️ IMPORTANT**: If you want to give agents read-only GitHub access via `GH_TOKEN`, you MUST provide `ZBOBR_COPILOT_GITHUB_TOKEN` so Copilot can create forks and push to its own session repos
- **Resolution Order**:
  1. `ZBOBR_COPILOT_GITHUB_TOKEN` environment variable
  2. `COPILOT_GITHUB_TOKEN` environment variable
  3. `GH_TOKEN` environment variable
  4. `GITHUB_TOKEN` environment variable
  5. `$(gh auth token)` (from gh CLI)
- **Config File**: `copilot_github_token` in `zbobr.toml`

#### Token Setup Example

**Scenario: Restrict agent GitHub access but keep Copilot working**

```bash
# Main token with full access (for zbobr orchestrator)
export GH_TOKEN=$(gh auth token)

# Read-only token for agents (create one with only 'public_repo' scope)
export ZBOBR_AGENT_GH_TOKEN=github_pat_xxxx_readonly

# Copilot's own token (can be same as GH_TOKEN, or a separate full-access token)
export ZBOBR_COPILOT_GITHUB_TOKEN=$(gh auth token)
```

**Scenario: All tokens from environment**

```bash
# Orchestrator & Copilot token
export GH_TOKEN=$(gh auth token)

# Read-only token for agents (different from GH_TOKEN)
export ZBOBR_AGENT_GH_TOKEN=github_pat_xxxx_readonly

# Copilot reuses GH_TOKEN if ZBOBR_COPILOT_GITHUB_TOKEN not set
# (or explicitly set it if different)
export ZBOBR_COPILOT_GITHUB_TOKEN=$GH_TOKEN
```

**Token Validation**: Zbobr validates at startup:
- `ZBOBR_AGENT_GH_TOKEN` must be set
- `ZBOBR_AGENT_GH_TOKEN` must be **different** from `ZBOBR_OWNER_GH_TOKEN` (prevents accidental write access)

## Important Notes

- **All issue/PR management** happens in the domain project repository
- **Agents never modify** target repositories directly—they only create forks and PRs
- **Labels and milestones** are orchestrator-managed and universal
- **Domain projects are configurable** — each domain defines its own target repositories
- **Fork owners can be users or organizations** — no GitHub org creation required

## Documentation

- [Domain Project Guide](DOMAIN_PROJECT.md) — How domain projects work (stages, labels, prompts, configuration)
- [Sample Configuration](zbobr.toml.sample) — Example `zbobr.toml` with all options
- [Manager Agent Instructions](.github/agents/manager.md) — Manager workflow
- [Worker Agent Instructions](.github/agents/worker.md) — Worker workflow
- [Agent Registry](AGENTS.md) — Agent definitions for `/agent` command

## Development

Build and test:

```bash
cargo build
cargo test
```
