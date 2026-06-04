```markdown
# meetily Development Patterns

> Auto-generated skill from repository analysis

## Overview
This skill teaches the core development patterns and conventions used in the `meetily` TypeScript codebase. You'll learn how to structure files, write imports and exports, follow commit message guidelines, and organize tests. These patterns ensure consistency and maintainability across the project.

## Coding Conventions

### File Naming
- Use **camelCase** for file names.
  - Example: `userProfile.ts`, `eventManager.ts`

### Import Style
- Use **relative imports** for referencing modules.
  - Example:
    ```typescript
    import { getUser } from './userService';
    ```

### Export Style
- Use **named exports** for all modules.
  - Example:
    ```typescript
    // In userService.ts
    export function getUser(id: string) { ... }
    ```

### Commit Messages
- Follow **Conventional Commits** style.
- Use prefixes such as `ci`.
- Keep commit messages concise (average ~46 characters).
  - Example:
    ```
    ci: update build pipeline for deployment
    ```

## Workflows

### Conventional Commit Workflow
**Trigger:** When making a commit
**Command:** `/conventional-commit`

1. Stage your changes.
2. Write a commit message using the format: `<type>: <short description>`
   - Example: `ci: add continuous integration config`
3. Commit your changes.

### Module Import/Export Workflow
**Trigger:** When creating or updating modules
**Command:** `/module-structure`

1. Name your file using camelCase.
2. Use relative imports to reference other modules.
3. Export functions or constants using named exports.

   Example:
   ```typescript
   // In eventManager.ts
   export function createEvent(data: EventData) { ... }
   ```

### Testing Workflow

**Trigger:** When writing or running tests
**Command:** `/run-tests`

1. Create test files with the pattern `*.test.*` (e.g., `userService.test.ts`).
2. Write tests using the project's preferred (undetected) testing framework.
3. Run tests using the project's test runner (check project scripts or documentation).

## Testing Patterns

- Test files follow the pattern: `*.test.*`
  - Example: `userService.test.ts`
- Place tests in a dedicated test directory (e.g., `tests/` or `__tests__/`), mirroring the source structure.
- Use the project's test runner to execute tests. (Framework is unspecified; check project scripts.)

## Commands

| Command               | Purpose                                         |
|-----------------------|-------------------------------------------------|
| /conventional-commit  | Guide for writing conventional commit messages  |
| /module-structure     | Steps for creating/importing/exporting modules  |
| /run-tests            | Instructions for writing and running tests      |
```
