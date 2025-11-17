# GitOps Cluster Environment Configurations

This directory contains FluxCD GitRepository configurations for different environments.

## Structure

```
gitops/cluster/env/
├── tilt/          # Tilt local development environment
├── dev/           # Development environment
├── stage/         # Staging environment
└── prod/          # Production environment
```

Each environment directory contains:
- `namespace.yaml` - Kubernetes namespace for the environment
- `gitrepository.yaml` - FluxCD GitRepository resource definition
- `secretmanagerconfig.yaml` - SecretManagerConfig resource for the environment
- `kustomization.yaml` - Kustomize configuration for the environment

## Usage

### Tilt Environment

The tilt environment is automatically applied when running `tilt up`. It includes:
- Namespace (`tilt`) for local development
- GitRepository pointing to the `secret-manager-controller` repository
- SecretManagerConfig (`test-sops-config`) in the `tilt` namespace

To apply manually:
```bash
kubectl apply -k gitops/cluster/env/tilt
```

This will create the namespace, GitRepository, and SecretManagerConfig resources, triggering reconciliation.

### Other Environments

For dev/stage/prod environments, update the GitRepository URL in each `gitrepository.yaml` file to point to your actual GitOps repositories.

To apply:
```bash
# Development
kubectl apply -k gitops/cluster/env/dev

# Staging
kubectl apply -k gitops/cluster/env/stage

# Production
kubectl apply -k gitops/cluster/env/prod
```

## GitRepository Configuration

Each GitRepository resource:
- Points to a Git repository containing deployment configurations
- Watches a specific branch (typically `main`)
- **Filters to only include environment-specific paths**:
  - Tilt: `deployment-configuration/profiles/tilt/` and `.sops.yaml`
  - Dev: `deployment-configuration/profiles/dev/` and `.sops.yaml`
  - Stage: `deployment-configuration/profiles/stage/` and `.sops.yaml`
  - Prod: `deployment-configuration/profiles/prod/` and `.sops.yaml`
- Checks for updates at configured intervals:
  - Tilt: 1 minute (for fast development iteration)
  - Dev/Stage: 5 minutes
  - Prod: 10 minutes (longer interval for stability)

### Path Filtering

Each GitRepository uses FluxCD's `ignore` field with gitignore patterns to filter the cloned repository. This ensures:
- **Security**: Only the relevant environment's configuration is cloned, preventing config bleed in case of cluster compromise
- `.sops.yaml` is included (required for SOPS decryption)
- Other environments' configurations are completely excluded
- Reduces clone size and improves performance

The filtering pattern (simplified and secure):
```yaml
ignore: |
  /*
  !/deployment-configuration/profiles/{env}/**
  !/.sops.yaml
```

This pattern:
1. Ignores everything at root (`/*`)
2. Includes only the specific profile directory for that environment (`!/deployment-configuration/profiles/{env}/**`)
3. Includes `.sops.yaml` at root (`!/.sops.yaml`)

**Security Note**: By only including the specific profile directory (not the entire `deployment-configuration` structure), we prevent accidental access to other environments' configurations even if there's a path traversal vulnerability or misconfiguration.

## SecretManagerConfig Resources

Each environment directory includes a `secretmanagerconfig.yaml` that:
- References the environment's GitRepository via `sourceRef`
- Is configured for that environment's profile (tilt, stage, prod)
- Uses appropriate reconcile intervals for the environment

**Example (tilt environment):**
```yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: test-sops-config
  namespace: tilt  # Same namespace as GitRepository
spec:
  sourceRef:
    kind: GitRepository
    name: gitrepository-tilt  # References the tilt GitRepository
    namespace: tilt  # Same namespace - GitRepository is in tilt namespace
  secrets:
    environment: tilt  # Matches the profile directory
  # ... rest of configuration
```

The SecretManagerConfig resources are automatically applied along with GitRepositories when you run `kubectl apply -k gitops/cluster/env/{env}`.

## Private Repositories

If your Git repositories are private, you'll need to create a secret with git credentials and reference it in the GitRepository:

```yaml
spec:
  secretRef:
    name: git-credentials
```

Create the secret:
```bash
kubectl create secret generic git-credentials \
  --from-literal=username=<your-username> \
  --from-literal=password=<your-token> \
  -n flux-system
```

For SSH authentication:
```bash
kubectl create secret generic git-ssh-credentials \
  --from-file=identity=<path-to-private-key> \
  -n flux-system
```

## Troubleshooting

### Check GitRepository Status

```bash
kubectl get gitrepository -n flux-system
kubectl describe gitrepository gitrepository-tilt -n tilt
```

### View GitRepository Logs

```bash
kubectl logs -n flux-system -l app=source-controller
```

### Verify Artifact Path

The FluxCD source-controller clones repositories to `/tmp/flux-source-<namespace>-<name>`. Verify the artifact is available:

```bash
kubectl get gitrepository gitrepository-tilt -n tilt -o jsonpath='{.status.artifact.path}'
```

