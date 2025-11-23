# GitOps Cluster Configurations

This directory contains GitOps configurations for different providers (FluxCD and ArgoCD) and environments.

## Structure

```
gitops/cluster/
├── fluxcd/          # FluxCD GitOps provider configurations
│   └── env/
│       ├── tilt/    # Tilt local development environment
│       ├── dev/     # Development environment
│       ├── stage/   # Staging environment
│       └── prod/    # Production environment
└── argocd/          # ArgoCD GitOps provider configurations
    └── env/
        ├── tilt/    # Tilt local development environment
        ├── dev/     # Development environment
        ├── stage/   # Staging environment
        └── prod/    # Production environment
```

## Provider Separation

Each GitOps provider has its own directory structure to:
- **Maintain separation of concerns** - FluxCD and ArgoCD resources are clearly separated
- **Enable independent testing** - Test each provider without interference
- **Support provider-specific configurations** - Each provider may have different requirements
- **Allow selective deployment** - Deploy FluxCD or ArgoCD independently

## FluxCD Structure

Each FluxCD environment directory contains:
- `namespace.yaml` - Kubernetes namespace for the environment
- `gitrepository.yaml` - FluxCD GitRepository resource definition
- `secretmanagerconfig.yaml` - SecretManagerConfig resource (references GitRepository)
- `kustomization.yaml` - Kustomize configuration

## ArgoCD Structure

Each ArgoCD environment directory contains:
- `namespace.yaml` - Kubernetes namespace for the environment
- `application.yaml` - ArgoCD Application resource definition
- `secretmanagerconfig.yaml` - SecretManagerConfig resource (references Application)
- `kustomization.yaml` - Kustomize configuration

## Usage

### FluxCD Environments

To apply a FluxCD environment:

```bash
# Tilt (local development)
kubectl apply -k gitops/cluster/fluxcd/env/tilt

# Development
kubectl apply -k gitops/cluster/fluxcd/env/dev

# Staging
kubectl apply -k gitops/cluster/fluxcd/env/stage

# Production
kubectl apply -k gitops/cluster/fluxcd/env/prod
```

### ArgoCD Environments

To apply an ArgoCD environment:

```bash
# Tilt (local development)
kubectl apply -k gitops/cluster/argocd/env/tilt

# Development
kubectl apply -k gitops/cluster/argocd/env/dev

# Staging
kubectl apply -k gitops/cluster/argocd/env/stage

# Production
kubectl apply -k gitops/cluster/argocd/env/prod
```

## SecretManagerConfig Source Reference

The `SecretManagerConfig` resource references the GitOps source via `sourceRef`:

**For FluxCD:**
```yaml
sourceRef:
  kind: GitRepository
  name: gitrepository-tilt
  namespace: tilt
```

**For ArgoCD:**
```yaml
sourceRef:
  kind: Application
  name: secret-manager-tilt
  namespace: tilt
```

## Testing

### Test FluxCD Only

```bash
# Install FluxCD
python3 scripts/tilt/install_fluxcd.py

# Apply FluxCD resources
kubectl apply -k gitops/cluster/fluxcd/env/tilt
```

### Test ArgoCD Only

```bash
# Install ArgoCD
python3 scripts/tilt/install_argocd.py

# Apply ArgoCD resources
kubectl apply -k gitops/cluster/argocd/env/tilt
```

### Test Both Providers

You can deploy both providers simultaneously - they operate independently:
- FluxCD GitRepositories are managed by FluxCD source-controller
- ArgoCD Applications are managed by ArgoCD application-controller
- SecretManagerConfig resources can reference either provider

## Migration from Old Structure

The old structure (`gitops/cluster/env/`) has been reorganized:
- FluxCD resources moved to `gitops/cluster/fluxcd/env/`
- ArgoCD resources moved to `gitops/cluster/argocd/env/`

Update any scripts or documentation that reference the old paths.

