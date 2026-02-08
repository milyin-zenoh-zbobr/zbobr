# Target Repositories

List the repositories this domain project manages. When issues are assigned to agents, zbobr will:

1. **Fork** these repositories to the `ZBOBR_FORK_OWNER` account (configured in `.zbobr.env`)
2. **Clone** the forks to the agent's workspace
3. **Create feature branches** for implementing changes
4. **Push commits** to the forks
5. **Open pull requests** back to the original repositories

## How to Configure

Replace the example repositories below with your actual target repositories. You can:

- List them as bullet points (as shown below)
- List them as plain URLs (one per line)
- Organize them into sections if you have many repositories

## Repositories

Replace the examples below with your actual target repositories:

- https://github.com/example/repo1
- https://github.com/example/repo2

## Example Configurations

### Single Repository
```markdown
## Repositories
- https://github.com/myorg/my-app
```

### Multiple Related Repositories
```markdown
## Repositories
- https://github.com/myorg/backend-api
- https://github.com/myorg/frontend-web
- https://github.com/myorg/mobile-app
- https://github.com/myorg/shared-utils
```

### Organized by Component
```markdown
## Backend Services
- https://github.com/myorg/auth-service
- https://github.com/myorg/payment-service

## Frontend Applications
- https://github.com/myorg/admin-dashboard
- https://github.com/myorg/customer-portal

## Infrastructure
- https://github.com/myorg/terraform-configs
- https://github.com/myorg/k8s-manifests
```

## Permissions Required

Ensure the GitHub token (`GH_TOKEN`) has:
- **Read access** to all target repositories (to clone and analyze)
- **Fork permission** (ability to fork to `ZBOBR_FORK_OWNER`)
- **Write access** to forks (to push branches)
- **PR creation access** (to open pull requests to original repos)

## Notes

- Agents will automatically determine which repository to work in based on issue context
- If an issue mentions multiple repositories, the agent may work across several forks
- Forks are reused across issues - agents pull latest changes before starting new work
