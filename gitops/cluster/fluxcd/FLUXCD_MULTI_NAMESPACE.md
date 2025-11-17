# FluxCD Multi-Namespace Configuration

## Overview

By default, FluxCD's source-controller only watches the `flux-system` namespace. To enable GitRepositories in other namespaces (tilt, dev, stage, prod), you need to configure FluxCD to watch multiple namespaces.

## Option 1: Configure FluxCD to Watch All Namespaces (Recommended)

Update the FluxCD source-controller deployment to watch all namespaces:

```bash
# Edit the source-controller deployment
kubectl edit deployment source-controller -n flux-system

# Add or update the --watch-all-namespaces flag:
spec:
  template:
    spec:
      containers:
      - name: manager
        args:
        - --watch-all-namespaces=true
```

Or patch it directly:

```bash
kubectl patch deployment source-controller -n flux-system --type='json' -p='[
  {
    "op": "add",
    "path": "/spec/template/spec/containers/0/args/-",
    "value": "--watch-all-namespaces=true"
  }
]'
```

## Option 2: Configure FluxCD to Watch Specific Namespaces

If you prefer to watch only specific namespaces:

```bash
kubectl patch deployment source-controller -n flux-system --type='json' -p='[
  {
    "op": "add",
    "path": "/spec/template/spec/containers/0/args/-",
    "value": "--watch-namespace=tilt"
  },
  {
    "op": "add",
    "path": "/spec/template/spec/containers/0/args/-",
    "value": "--watch-namespace=dev"
  },
  {
    "op": "add",
    "path": "/spec/template/spec/containers/0/args/-",
    "value": "--watch-namespace=stage"
  },
  {
    "op": "add",
    "path": "/spec/template/spec/containers/0/args/-",
    "value": "--watch-namespace=prod"
  }
]'
```

## Option 3: Use FluxCD Kustomization (GitOps Approach)

Create a FluxCD Kustomization resource that manages the source-controller configuration:

```yaml
apiVersion: kustomize.toolkit.fluxcd.io/v1
kind: Kustomization
metadata:
  name: source-controller-config
  namespace: flux-system
spec:
  sourceRef:
    kind: GitRepository
    name: flux-system
  path: ./flux-system/source-controller
  interval: 10m
```

Then manage the source-controller deployment via GitOps.

## Verification

After configuring FluxCD, verify it can see GitRepositories in other namespaces:

```bash
# Check if source-controller is watching all namespaces
kubectl get deployment source-controller -n flux-system -o yaml | grep -A 5 "args:"

# Verify GitRepository in tilt namespace gets processed
kubectl get gitrepository gitrepository-tilt -n tilt
kubectl get gitrepository gitrepository-tilt -n tilt -o jsonpath='{.status.conditions[?(@.type=="Ready")].status}'
```

## Note

If you don't configure FluxCD to watch other namespaces, GitRepositories in those namespaces will not be processed and will not have artifacts. The controller will show errors like "GitRepository has no artifact in status".

