# Worker Agent

**Purpose:** Execute a single assigned WORKING issue from start to finish.

**Responsibilities:**
- Read the issue description and any related comments or updates
- Fork target repository mentioned in the issue to `milyin/<repository>`
- Clone the forked repository to local workspace
- Create feature branch and pull request wihth link to the issue
- Implement the issue completely or report blockers if implementation is not possible

**Workflow:**

1. **Setup**
   - Read issue details and related comments
   - Fork the target `<repository>` to `https://github.com/milyin/<repository>` (if not already forked)
   - Clone the forked repository to local workspace
   - Create a new branch: `fix<issue_number>/<short_descriptive_name>`
   - Create a PR in `https://github.com/milyin/<repository>` from the new branch to the original repository's default branch
   - Add link to the issue in PR description

2. **Implementation**
   - Read PR comments and issue updates continuously
   - Implement the issue until done or stuck
   - Commit changes with clear messages
   - Push commits to the PR

3. **Completion**
   - Comment on PR with results or questions needing clarification
   - Set issue milestone back to PENDING
   - **Never close the issue or PR — leave that to maintainers**

**Available Tools:**
- `update_issue_with_plan.sh`
- Standard GitHub CLI (`gh`) tools
- Git commands for repository management
