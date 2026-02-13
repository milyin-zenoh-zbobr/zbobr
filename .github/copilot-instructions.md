Project Copilot Instructions
=============================

Add the following project-wide rules for Copilot / agent-related changes:

1) Avoid repeated string literals
- If a string literal is used in multiple places, prefer a `const` or an `enum` with a single canonical representation.
- Rationale: prevents accidental divergence and makes refactors safer.
- Example (Rust):

  pub const ISSUE_LABEL_BUG: &str = "bug";

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum Tool { Copilot, Claude, Stub }

  impl std::fmt::Display for Tool {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
          match self {
              Tool::Copilot => write!(f, "copilot"),
              Tool::Claude => write!(f, "claude"),
              Tool::Stub => write!(f, "stub"),
          }
      }
  }

2) Signals are processed on the coordinator level
- All control signals (stage transitions, cancel, pause, reassign, etc.) must be handled by the coordinator process. Agents should not implement independent signal-handling logic or rely on receiving those signals directly.
- Rationale: keeps agent implementations simple and ensures a single source of truth for orchestration decisions.
- Implementation note: Coordinator MCP and orchestration code (see [zbobr-lib/src/mcp.rs](zbobr-lib/src/mcp.rs) and coordinator entrypoint [zbobr/src/main.rs](zbobr/src/main.rs)) should own signal handling and translate changes into agent-facing messages or stage updates.

3) Run `gh` from the coordinator only as a last resort
- Prefer using the `octocrab` Rust library (or other supported GitHub API client) for all GitHub API interactions executed by the coordinator.
- Only execute the `gh` CLI from the coordinator when the required operation cannot be achieved with `octocrab` (for example, a feature that is only available via the CLI or a one-off administrative action). Document the reason and the exact CLI command in a code comment or changelog entry when falling back to `gh`.
- Rationale: using the API client keeps interactions testable, reproducible, and avoids shelling out where a stable library exists.

4) Keep setup script in sync with stage and label changes
- When adding, removing, or modifying task stages (milestones) or labels, update [zbobr-lib/src/backend/github.rs](zbobr-lib/src/backend/github.rs) in the `setup_repository` function to match.
- Rationale: ensures the setup command continues to properly initialize repositories with all necessary stages and labels.

Where to apply these rules
- New code and edits touching agent tooling, task orchestration, GitHub integrations, or shared constants should follow these guidelines.
- Helpful code locations: [zbobr-lib/src/config.rs](zbobr-lib/src/config.rs), [zbobr-lib/src/tool_executor.rs](zbobr-lib/src/tool_executor.rs), [zbobr-lib/src/mcp.rs](zbobr-lib/src/mcp.rs), and the coordinator binary at [zbobr/src/main.rs](zbobr/src/main.rs).
