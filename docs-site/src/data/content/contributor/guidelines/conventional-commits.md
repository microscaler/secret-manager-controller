# Conventional Commits

This project follows the [Conventional Commits](https://www.conventionalcommits.org/) specification for commit messages. This ensures consistent, readable commit history and enables automated tooling.

## Format

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

## Commit Types

### `feat`
A new feature.

**Examples:**
```
feat: add support for Azure Key Vault
feat(aws): implement Parameter Store integration
feat(cli): add reconcile command to msmctl
```

### `fix`
A bug fix.

**Examples:**
```
fix: correct secret name validation
fix(gcp): handle missing project ID gracefully
fix(parser): resolve SOPS decryption edge case
```

### `docs`
Documentation only changes.

**Examples:**
```
docs: update installation guide
docs(api): add CRD reference documentation
docs: fix typo in SOPS setup guide
```

### `style`
Changes that do not affect the meaning of the code (white-space, formatting, missing semi-colons, etc).

**Examples:**
```
style: format code with cargo fmt
style(parser): improve code readability
```

### `refactor`
A code change that neither fixes a bug nor adds a feature.

**Examples:**
```
refactor: reorganize provider modules
refactor(parser): simplify file parsing logic
refactor(reconciler): extract common validation logic
```

### `perf`
A code change that improves performance.

**Examples:**
```
perf: optimize secret sync operations
perf(parser): cache parsed file contents
```

### `test`
Adding missing tests or correcting existing tests.

**Examples:**
```
test: add integration tests for GCP provider
test(aws): fix flaky parameter store test
test: increase coverage for parser module
```

### `build`
Changes that affect the build system or external dependencies (example scopes: cargo, docker, tilt).

**Examples:**
```
build: update Rust toolchain to 1.75
build(docker): optimize base image size
build(tilt): add docs-site resource
```

### `ci`
Changes to CI configuration files and scripts.

**Examples:**
```
ci: add GitHub Actions workflow
ci: fix Kind cluster setup in CI
ci: update Pact test execution
```

### `chore`
Other changes that don't modify src or test files (maintenance tasks, tooling updates, etc).

**Examples:**
```
chore: update dependencies
chore: remove unused scripts
chore: update .gitignore
```

### `revert`
Reverts a previous commit.

**Examples:**
```
revert: revert "feat: add Azure App Configuration support"
revert(aws): revert Parameter Store changes
```

## Scope

The scope is optional and should be the area of the codebase affected by the change.

**Common scopes:**
- `aws`, `gcp`, `azure` - Provider-specific changes
- `parser` - File parsing logic
- `reconciler` - Reconciliation logic
- `cli` - Command-line interface
- `docs` - Documentation
- `test` - Testing
- `ci` - CI/CD
- `build` - Build system

**Examples:**
```
feat(aws): add Parameter Store support
fix(gcp): correct authentication flow
docs(cli): update msmctl usage guide
```

## Description

The description should:
- Be written in imperative mood ("add feature" not "added feature" or "adds feature")
- Start with a lowercase letter (unless using breaking change indicator)
- Not end with a period
- Be concise but descriptive (minimum 10 characters)

**Good:**
```
feat: add support for AGE encryption keys
fix(aws): handle missing region gracefully
docs: update SOPS setup guide
```

**Bad:**
```
feat: Added new feature.
fix: bug fix
docs: docs
```

## Breaking Changes

Use `!` after the type/scope to indicate a breaking change.

**Format:**
```
<type>[optional scope]!: <description>
```

**Examples:**
```
feat!: change CRD API version to v1
refactor(parser)!: restructure file parsing API
```

Breaking changes should also include a `BREAKING CHANGE:` footer:

```
feat!: change CRD API version to v1

BREAKING CHANGE: The SecretManagerConfig CRD now uses v1 API version.
Previous v1alpha1 resources must be migrated.
```

## Body

The body is optional and should:
- Provide additional context about the change
- Explain the "what" and "why" (not the "how")
- Be separated from the description by a blank line
- Wrap at 72 characters

**Example:**
```
feat: add support for AGE encryption keys

AGE (Actually Good Encryption) is a modern alternative to GPG
for SOPS encryption. This adds support for AGE keys alongside
existing GPG key support, providing users with more flexibility
in their encryption setup.
```

## Footer

Footers are optional and can include:
- Breaking changes: `BREAKING CHANGE: <description>`
- Issue references: `Closes #123`, `Fixes #456`
- Co-authors: `Co-authored-by: Name <email>`

**Example:**
```
fix(aws): correct secret name validation

The validation logic was incorrectly rejecting valid secret names
that contained hyphens. This fix updates the regex pattern to
allow hyphens in secret names.

Fixes #123
Closes #456
```

## Commit Message Hook

A Git hook automatically validates commit messages to ensure they follow the Conventional Commits specification.

**Validation checks:**
- ✅ Format matches `<type>[scope]: <description>`
- ✅ Type is one of the allowed types
- ✅ Description is at least 10 characters
- ✅ Description starts with lowercase (unless breaking change)

**If validation fails:**
The commit will be rejected with an error message showing:
- What format is expected
- Examples of valid commit messages
- Your invalid message

**Bypassing the hook (not recommended):**
```bash
# Use --no-verify flag (only for emergencies)
git commit --no-verify -m "message"
```

## Examples

### Simple Feature
```
feat: add support for Azure Key Vault
```

### Feature with Scope
```
feat(aws): implement Parameter Store integration
```

### Bug Fix
```
fix(gcp): handle missing project ID gracefully
```

### Documentation Update
```
docs: update installation guide with new prerequisites
```

### Breaking Change
```
feat!: change CRD API version to v1

BREAKING CHANGE: The SecretManagerConfig CRD now uses v1 API version.
Previous v1alpha1 resources must be migrated to the new version.
```

### Complex Change with Body and Footer
```
feat(parser): add support for application.properties routing

When configs.enabled=true, application.properties files are now
routed to config stores (Parameter Store, App Configuration)
instead of secret stores. This provides better separation of
secrets and configuration values.

Closes #789
```

### Refactoring
```
refactor(reconciler): extract common validation logic

Moved shared validation functions to a common module to reduce
code duplication across provider-specific reconcilers.
```

### Test Addition
```
test: add integration tests for GCP provider

Adds comprehensive integration tests covering:
- Secret creation and updates
- SOPS decryption
- Error handling scenarios
```

## Benefits

Following Conventional Commits provides:

1. **Automated Changelog Generation**: Tools can automatically generate changelogs from commit history
2. **Semantic Versioning**: Commit types can drive version bumps (feat = minor, fix = patch, breaking = major)
3. **Better Git History**: Easy to search and filter commits by type
4. **Clear Communication**: Commit messages clearly communicate the intent of changes
5. **Tooling Integration**: Works with tools like semantic-release, commitlint, etc.

## Resources

- [Conventional Commits Specification](https://www.conventionalcommits.org/)
- [Angular Commit Message Guidelines](https://github.com/angular/angular/blob/main/CONTRIBUTING.md#commit)
- [Commit Message Best Practices](https://chris.beams.io/posts/git-commit/)

## Next Steps

- [Code Style](./code-style.md) - Code formatting and style guidelines
- [Error Handling](./error-handling.md) - Error handling patterns
- [Logging](./logging.md) - Logging guidelines

