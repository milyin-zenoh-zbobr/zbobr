# Domain Project Instructions

This repository manages issues for a specific project/domain using the Copilot Orchestrator automation system.

## Quick Start

See [repositories.md](repositories.md) for the list of target repositories managed by this domain project.

## Workflow

Issues flow through these stages:

1. **PLANNING** - Manager researches and creates an implementation plan
2. **PENDING** - Waiting for human review
3. **READY** - Approved and ready for implementation
4. **WORKING** - Worker agent is implementing the issue

Add `model:` labels to control which AI model is used (defaults to `gpt-5-mini`).

## Resources

- [Copilot Orchestrator](https://github.com/milyin/copilot) - The automation system
- Each issue links to its implementation PR via GitHub's issue-PR backlinks
- See orchestrator's `automation/agents/*.md` for agent behavior details
