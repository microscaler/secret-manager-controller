# Agent Guide - Secret Manager Controller

This guide helps AI agents understand how to work with the Secret Manager Controller.

## 🎯 Quick Start

1. **Check GitHub Issues First** - Always review open issues before starting work
2. **This is a Kubernetes Controller** - Built with Rust, runs in Kubernetes clusters
3. **GitOps Integration** - Syncs secrets from Git to GCP Secret Manager
4. **Update Issues** - Keep issues updated with progress and findings

## 📋 Working with Issues

### Finding Relevant Issues

```bash
# List all open issues for this controller
gh issue list --repo microscaler/secret-manager-controller

# Search for specific issues
gh issue list --repo microscaler/secret-manager-controller --label "bug"
gh issue list --repo microscaler/secret-manager-controller --label "enhancement"
```

### Before Starting Work

1. **Check Open Issues** - Review GitHub issues before starting
2. **Understand the Controller** - Read `README.md` for architecture and usage
3. **Review Recent Changes** - Check git log and recent PRs
4. **Understand Kubernetes** - Know CRDs, controllers, and reconciliation

## 🛠️ Development Workflow

### 1. Create or Assign Issue

```bash
# Create new issue
gh issue create \
  --repo microscaler/secret-manager-controller \
  --title "Issue title" \
  --body-file issue-body.md \
  --label "bug"

# Assign issue to yourself
gh issue edit <ISSUE_NUMBER> \
  --repo microscaler/secret-manager-controller \
  --add-assignee @me
```

### 2. Create Branch

```bash
git checkout -b fix/issue-<NUMBER>-short-description
```

### 3. Make Changes

- Follow Rust best practices
- Test locally with `cargo test`
- Build with `cargo build --release`
- Test Kubernetes deployment: `kubectl apply -f config/`
- Test with Tilt: `tilt up` (if Tiltfile exists)
- **Never commit GPG keys** - They're gitignored for a reason!

### 4. Update Issue

```bash
gh issue comment <ISSUE_NUMBER> \
  --repo microscaler/secret-manager-controller \
  --body "Progress update: Implemented X, testing Y"
```

### 5. Create PR

```bash
gh pr create \
  --repo microscaler/secret-manager-controller \
  --title "Fix: Issue title" \
  --body "Fixes #<ISSUE_NUMBER>"
```

## 📚 Key Files

- `README.md` - Main documentation
- `src/main.rs` - Main controller code
- `src/reconciler.rs` - Reconciliation logic
- `src/gcp.rs` - GCP Secret Manager integration
- `src/kustomize.rs` - Kustomize build support
- `src/parser.rs` - Secret file parsing
- `config/` - Kubernetes manifests
- `Dockerfile` - Container build
- `Cargo.toml` - Rust dependencies

## 🚨 Important Rules

1. **Always check issues first** - Don't duplicate work
2. **Test locally** - Use `cargo test` and `cargo run`
3. **Test Kubernetes** - Deploy to kind cluster for testing
4. **Never commit GPG keys** - They're in .gitignore
5. **Update documentation** - Keep README.md current
6. **Follow Rust conventions** - Use `cargo fmt` and `cargo clippy`
7. **Understand GitOps** - Know how GitOps reconciliation works
8. **Understand SOPS** - Know how SOPS encryption/decryption works

## 🔗 Related Resources

- **Repository:** https://github.com/microscaler/secret-manager-controller
- **Kubernetes Docs:** https://kubernetes.io/docs/
- **Kube-RS Docs:** https://docs.rs/kube/
- **GCP Secret Manager:** https://cloud.google.com/secret-manager
- **SOPS Docs:** https://github.com/getsops/sops

## ✅ Checklist Before PR

- [ ] Issue created or assigned
- [ ] Branch created from main
- [ ] Changes made and tested
- [ ] `cargo test` passes
- [ ] `cargo build --release` succeeds
- [ ] Kubernetes manifests tested
- [ ] No GPG keys committed
- [ ] Documentation updated
- [ ] Issue updated with progress
- [ ] PR created with issue reference

---

**Remember:** Always check issues first, update them as you work, and link PRs to issues!

