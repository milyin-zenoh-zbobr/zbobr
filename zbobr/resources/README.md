# Domain Project

This repository is managed by [zbobr](https://github.com/milyin/zbobr) -- an AI-powered issue orchestrator.

Issues in this repository flow through automated stages. Planner and worker agents process them via MCP tools, while humans review and approve at key checkpoints.

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

See `.zbobr.env` for environment variables and `run.sh` / `run.cmd` for launcher scripts.

## Target Repositories

List the repositories this domain project manages in `repositories.md`.
Workers will fork these repositories to implement issues.
