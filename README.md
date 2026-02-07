# Copilot Agent Workflow

AI-powered issue management system using GitHub Copilot agents to plan, coordinate, and implement fixes for the [Zenoh project ecosystem](REPOSITORIES.md).

## Overview

This repository manages a multi-agent workflow:

- **Manager Agent**: Processes issues through stages (PLANNING → PENDING → READY → WORKING), creates implementation plans, and spawns Worker agents
- **Worker Agent**: Implements individual issues by forking repos to `milyin/*`, creating PRs, and executing the work

## Getting Started

**Prerequisites:**
- GitHub Copilot CLI (`copilot`)
- GitHub CLI (`gh`) authenticated
- Access to `milyin/*` GitHub account

**Launch agents:**
```bash
# Start Manager to process issues
copilot --agent manager -i "Process issues using the manager workflow."

# Start Worker for specific issue
copilot --agent worker --model gpt-5.2-codex -i "Fix issue #123 in milyin/copilot."
```

## Workflow

1. Create issues in `milyin/copilot` with milestone `PLANNING`
2. Manager creates implementation plan → sets milestone `PENDING`
3. Human reviews and sets milestone to `READY`
4. Manager spawns Worker → sets milestone `WORKING`
5. Worker forks target repo to `milyin/*`, creates PR, implements fix
6. Worker sets milestone to `PENDING` + adds `done` label when complete
7. Human reviews PR and merges

See [.github/copilot-instructions.md](.github/copilot-instructions.md) for detailed documentation.

## Target Repositories

This workflow manages issues related to the Zenoh ecosystem. See [REPOSITORIES.md](REPOSITORIES.md) for the full list of target repositories.

**Important:** All issue and PR management happens in `milyin/copilot`. Agents never directly modify other repositories—they only create forks and PRs.
