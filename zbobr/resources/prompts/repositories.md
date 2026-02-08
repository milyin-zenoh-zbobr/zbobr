# Target Repositories

This file lists the repositories that agents can work on. This information is automatically included for planner agents to help them understand which repositories are available for implementation.

## Repositories

List the repositories this domain project manages:

- https://github.com/example/backend-api
- https://github.com/example/frontend-app
- https://github.com/example/shared-library

## Repository Notes

Add any repository-specific notes here:

### example/backend-api
- Main REST API service
- Uses Rust with axum framework
- Database migrations in `migrations/` directory

### example/frontend-app
- React-based web application
- Uses TypeScript with Vite
- Component library in `src/components/`

### example/shared-library
- Shared utilities and types
- Used by both backend and frontend
- Must maintain backward compatibility

## Guidelines

When working with these repositories:
1. Fork the repository to the configured ZBOBR_FORK_OWNER
2. Create a feature branch following the naming convention
3. Implement changes according to the issue requirements
4. Run tests before creating PR
5. Create PR with descriptive title and body
