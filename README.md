# Secret Manager Controller

Kubernetes controller that syncs secrets from Flux GitRepositories to Google Cloud Secret Manager.

## Overview

This controller:

1. **GitOps-Agnostic Source Support** - Supports both FluxCD GitRepository and ArgoCD Application via `sourceRef` pattern
2. **Supports two modes:**
   - **Kustomize Build Mode** (recommended): Runs `kustomize build` and extracts secrets from generated Kubernetes Secret resources. Supports overlays, patches, and generators. Works with any GitOps tool (FluxCD, ArgoCD, etc.)
   - **Raw File Mode**: Reads `application.secrets.env`, `application.secrets.yaml`, and `application.properties` directly
3. **Decrypts SOPS files** - Automatically decrypts SOPS-encrypted secret files using GPG private key from Kubernetes secret
4. **GitOps-agnostic** - Works with FluxCD, ArgoCD, or any GitOps tool that provides GitRepository artifacts
5. **Stores in GCP Secret Manager** - Syncs secrets to Google Cloud Secret Manager for CloudRun consumption

## Architecture

```
GitRepository (Flux) → SourceController → Artifact Cache
                                         ↓
                    SecretManagerConfig (CRD)
                                         ↓
                    Secret Manager Controller
                    ├─ HTTP Server (metrics, probes)
                    ├─ SOPS Decryption
                    └─ GitOps Reconciliation
                                         ↓
                    Google Cloud Secret Manager
                    (Git is source of truth)
                                         ↓
                    CloudRun Services/Jobs (via Upbound Provider)
```

## GitOps Reconciliation

The controller implements **GitOps-style reconciliation** where Git is the source of truth:

1. **Git → GCP Sync**: When secrets change in Git, they are automatically synced to GCP Secret Manager
2. **Manual Changes Overwritten**: If a secret is manually changed in GCP Secret Manager, the controller will detect the difference and overwrite it with the Git value on the next reconciliation
3. **Version Management**: New versions are created in GCP Secret Manager when values change; old versions remain accessible but the new version becomes "latest"
4. **Continuous Reconciliation**: The controller continuously reconciles to ensure GCP Secret Manager matches Git

## CRD Definition

```yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: my-service-secrets
  namespace: default
spec:
  sourceRef:
    kind: GitRepository  # FluxCD GitRepository (default) or "Application" for ArgoCD
    name: my-repo
    namespace: flux-system
  gcpProjectId: my-gcp-project
  environment: dev  # Required - must match directory name under profiles/
  # Option 1: Kustomize Build Mode (recommended - supports overlays/patches)
  kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
  # Option 2: Raw File Mode (if kustomizePath not specified)
  # basePath: microservices  # Optional - if omitted, searches from repository root
  secretPrefix: my-service  # Optional, defaults to service name
  secretSuffix: -prod       # Optional, matches kustomize-google-secret-manager behavior
```

**Important:** 
- The `environment` field is **required** and must exactly match the directory name under `profiles/`. This allows the controller to explicitly sync a specific environment rather than scanning all environments. This is especially useful for projects using Skaffold with custom environment names like `dev-cf`, `pp-cf`, `prod-cf`.
- **Kustomize Build Mode** (when `kustomizePath` is specified): The controller runs `kustomize build` and extracts secrets from the generated Kubernetes Secret resources. This ensures overlays, patches, and generator modifications are included. Works with any GitOps tool (FluxCD, ArgoCD, etc.).
- **Raw File Mode** (when `kustomizePath` is not specified): The controller reads `application.secrets.env` files directly. Simpler but doesn't support kustomize overlays/patches.

## Directory Structure

The controller expects the following structure (matching Flux kustomize):

```
microservices/
  {service-name}/
    deployment-configuration/
      {environment}/
        application.secrets.env      # ENV format secrets
        application.secrets.yaml     # YAML format secrets
        application.properties       # Properties file
```

## Secret Naming

Secrets in GCP Secret Manager are named using the same convention as `kustomize-google-secret-manager` for drop-in replacement compatibility:

- `{secretPrefix}-{key}-{secretSuffix}` if both prefix and suffix are specified
- `{secretPrefix}-{key}` if only prefix is specified
- `{key}-{secretSuffix}` if only suffix is specified
- `{key}` if neither is specified

Invalid characters (`.`, `/`, spaces) are automatically replaced with `_` to comply with GCP Secret Manager naming requirements.

Examples:
- With prefix only: `my-service-database-url`, `my-service-api-key`
- With prefix and suffix: `my-service-database-url-prod`, `my-service-api-key-prod`
- Properties secret: `my-service-properties` or `my-service-properties-prod`

## Usage with CloudRun

After secrets are synced to GCP Secret Manager, use the [Upbound GCP CloudRun Provider](https://marketplace.upbound.io/providers/upbound/provider-gcp-cloudrun/v2.2.0) to reference them:

```yaml
apiVersion: cloudrun.gcp.upbound.io/v1beta1
kind: Service
metadata:
  name: my-service
spec:
  forProvider:
    template:
      spec:
        containers:
        - image: gcr.io/my-project/my-service:latest
          env:
          - name: DATABASE_URL
            valueFrom:
              secretKeyRef:
                name: my-service-database-url
                key: latest
```

## Development

### Prerequisites

- Rust 1.75+
- Kubernetes cluster with Flux installed (or any GitOps tool that provides GitRepository artifacts)
- GCP credentials configured
- `google-cloud-secret-manager` Rust crate dependencies
- **For Kustomize Build Mode**: `kustomize` binary must be available in the controller container (v5.0+)

### Build

```bash
cargo build --release
```

### Run Locally

```bash
# Set GCP credentials
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json

# Run controller
cargo run
```

### Deploy to Kubernetes

The controller deploys to the `flux-system` namespace (same as FluxCD) since it's heavily reliant on FluxCD for GitRepository resources.

```bash
# Apply CRD
kubectl apply -f config/crd/

# Deploy controller to flux-system namespace
kubectl apply -f config/deployment/
```

**Note:** The controller watches `SecretManagerConfig` resources in **all namespaces**, so you can deploy your `SecretManagerConfig` resources in any namespace where your services are deployed.

## Configuration

### Environment Variables

- `GOOGLE_APPLICATION_CREDENTIALS` - Path to GCP service account JSON
- `RUST_LOG` - Logging level (default: `info`)
- `METRICS_PORT` - Port for metrics and probe endpoints (default: `8080`)

### HTTP Endpoints

- `GET /metrics` - Prometheus metrics endpoint
- `GET /healthz` - Kubernetes liveness probe
- `GET /readyz` - Kubernetes readiness probe

### Prometheus Metrics

The controller exposes the following metrics:

- `secret_manager_reconciliations_total` - Total number of reconciliations
- `secret_manager_reconciliation_errors_total` - Total number of reconciliation errors
- `secret_manager_reconciliation_duration_seconds` - Duration of reconciliations
- `secret_manager_secrets_synced_total` - Total number of secrets synced to GCP
- `secret_manager_secrets_updated_total` - Total number of secrets updated (overwritten from git)
- `secret_manager_secrets_managed` - Current number of secrets being managed
- `secret_manager_gcp_operations_total` - Total number of GCP Secret Manager operations
- `secret_manager_gcp_operation_duration_seconds` - Duration of GCP operations

### SOPS Private Key

The controller automatically loads the SOPS private key from a Kubernetes secret in the `flux-system` namespace. It looks for secrets named:
- `sops-private-key`
- `sops-gpg-key`
- `gpg-key`

The secret should contain the private key in one of these data keys:
- `private-key`
- `key`
- `gpg-key`

**Example Secret:**

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: sops-private-key
  namespace: flux-system
type: Opaque
stringData:
  private-key: |
    -----BEGIN PGP PRIVATE KEY BLOCK-----
    ...
    -----END PGP PRIVATE KEY BLOCK-----
```

**Note:** This key should be the same GPG key used by the GitHub SOPS Bot and should be added to `.sops.yaml` and re-encrypted so the controller can decrypt files.

### Source Reference Support

The controller supports multiple GitOps tools via the `sourceRef` pattern:

**FluxCD GitRepository:**
```yaml
sourceRef:
  kind: GitRepository  # Default, can be omitted
  name: my-repo
  namespace: flux-system
```
- Uses FluxCD SourceController artifacts
- Artifact path extracted from GitRepository status
- Fully supported

**ArgoCD Application:**
```yaml
sourceRef:
  kind: Application
  name: my-app
  namespace: argocd
```
- Extracts Git source from Application spec
- **Note:** ArgoCD support requires Git repository access configuration
- May need to clone repository or access ArgoCD's repository cache
- See examples for ArgoCD configuration

### RBAC

The controller uses a `ClusterRole` to watch resources across all namespaces:
- `get`, `list`, `watch` on `secretmanagerconfigs.secret-management.microscaler.io` (all namespaces)
- `update`, `patch` on `secretmanagerconfigs.secret-management.microscaler.io/status` (all namespaces)
- `get`, `list`, `watch` on `gitrepositories.source.toolkit.fluxcd.io` (all namespaces) - FluxCD
- `get`, `list`, `watch` on `applications.argoproj.io` (all namespaces) - ArgoCD
- `get` on `secrets` in `flux-system` namespace (for SOPS private key)

**Namespace Flexibility:**
- Controller deploys to `flux-system` namespace
- `SecretManagerConfig` resources can be deployed in **any namespace**
- Controller automatically watches and reconciles resources in all namespaces

## MSMCTL CLI

`msmctl` (Microscaler Secret Manager Controller) is a command-line tool for interacting with the Secret Manager Controller running in Kubernetes. Similar to `fluxctl`, it provides commands to trigger reconciliations and view the status of SecretManagerConfig resources.

### Installation

#### Build from Source

```bash
cd hack/controllers/secret-manager-controller
cargo build --release --bin msmctl
```

The binary will be available at `target/release/msmctl`.

#### Prerequisites

- Kubernetes cluster with Secret Manager Controller deployed
- `kubectl` configured with access to the cluster
- RBAC permissions to read/update SecretManagerConfig resources

### Authentication

`msmctl` uses Kubernetes authentication primitives:

- **kubeconfig**: Uses the default kubeconfig (`~/.kube/config`) or `KUBECONFIG` environment variable
- **Service Account**: When running in-cluster, uses the pod's service account token
- **Client Certificates**: Supports client certificate authentication from kubeconfig

No additional authentication is required - `msmctl` leverages Kubernetes' built-in security mechanisms.

### Commands

#### `msmctl reconcile`

Trigger a manual reconciliation for a SecretManagerConfig resource.

**Usage:**
```bash
msmctl reconcile --name <name> [--namespace <namespace>]
```

**Options:**
- `--name, -n`: Name of the SecretManagerConfig resource (required)
- `--namespace, -N`: Namespace of the resource (defaults to "default")

**Example:**
```bash
# Trigger reconciliation in default namespace
msmctl reconcile --name idam-dev-secrets

# Trigger reconciliation in specific namespace
msmctl reconcile --name idam-dev-secrets --namespace pricewhisperer
```

**How it works:**
- Updates the `secret-management.microscaler.io/reconcile` annotation with a timestamp
- The controller watches for annotation changes and triggers reconciliation
- This is a Kubernetes-native approach that doesn't require HTTP endpoints

#### `msmctl list`

List all SecretManagerConfig resources.

**Usage:**
```bash
msmctl list [--namespace <namespace>]
```

**Options:**
- `--namespace, -N`: Namespace to list resources in (defaults to all namespaces)

**Example:**
```bash
# List all resources in all namespaces
msmctl list

# List resources in specific namespace
msmctl list --namespace pricewhisperer
```

**Output:**
```
NAME                          NAMESPACE            READY           SECRETS SYNCED
idam-dev-secrets             pricewhisperer       True            5
idam-prd-secrets             pricewhisperer       True            5
```

#### `msmctl status`

Show detailed status of a SecretManagerConfig resource.

**Usage:**
```bash
msmctl status --name <name> [--namespace <namespace>]
```

**Options:**
- `--name, -n`: Name of the SecretManagerConfig resource (required)
- `--namespace, -N`: Namespace of the resource (defaults to "default")

**Example:**
```bash
msmctl status --name idam-dev-secrets --namespace pricewhisperer
```

**Output:**
```
Status for SecretManagerConfig 'pricewhisperer/idam-dev-secrets':

Metadata:
  Name: idam-dev-secrets
  Namespace: pricewhisperer
  Generation: 1

Spec:
  GCP Project ID: pricewhisperer-dev
  Environment: dev
  Source: GitRepository/pricewhisperer-manifests
  Kustomize Path: microservices/idam/deployment-configuration/profiles/dev
  Secret Prefix: idam-dev
  Secret Suffix: -prod

Status:
  Observed Generation: 1
  Last Reconcile Time: 2024-01-15T10:30:00Z
  Secrets Synced: 5

Conditions:
  Ready: True
    Reason: ReconciliationSucceeded
    Message: Synced 5 secrets
    Last Transition: 2024-01-15T10:30:00Z
```

### Security

#### RBAC Requirements

The user/service account running `msmctl` needs the following permissions:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: msmctl-user
rules:
- apiGroups: ["secret-management.microscaler.io"]
  resources: ["secretmanagerconfigs"]
  verbs: ["get", "list", "patch"]
- apiGroups: ["secret-management.microscaler.io"]
  resources: ["secretmanagerconfigs/status"]
  verbs: ["get"]
```

#### Authentication Methods

1. **kubeconfig** (default): Uses credentials from `~/.kube/config`
2. **Service Account**: When running in-cluster, automatically uses pod service account
3. **Environment Variables**: Can use `KUBECONFIG` to specify custom config file

### Comparison with fluxctl

| Feature | fluxctl | msmctl |
|---------|---------|--------|
| **Trigger Reconciliation** | ✅ | ✅ |
| **List Resources** | ✅ | ✅ |
| **Show Status** | ✅ | ✅ |
| **Authentication** | kubeconfig | kubeconfig |
| **Method** | HTTP endpoint | Annotation-based |

#### Advantages of Annotation-Based Approach

- **Kubernetes-native**: Uses standard Kubernetes watch mechanisms
- **No HTTP endpoint required**: Controller doesn't need to expose HTTP API
- **Audit trail**: Annotation changes are logged in Kubernetes audit logs
- **RBAC integration**: Uses standard Kubernetes RBAC for authorization

### Troubleshooting

#### "Failed to create Kubernetes client"

**Cause:** kubeconfig not configured or invalid

**Solution:**
```bash
# Verify kubeconfig
kubectl cluster-info

# Set KUBECONFIG if needed
export KUBECONFIG=/path/to/kubeconfig
```

#### "Failed to trigger reconciliation"

**Cause:** Insufficient RBAC permissions

**Solution:** Ensure your user/service account has `patch` permission on SecretManagerConfig resources.

#### "Resource not found"

**Cause:** SecretManagerConfig doesn't exist or wrong namespace

**Solution:**
```bash
# List resources to verify name and namespace
msmctl list

# Use correct namespace
msmctl reconcile --name <name> --namespace <correct-namespace>
```

### Examples

#### CI/CD Integration

Trigger reconciliation after deploying secrets:

```bash
#!/bin/bash
# deploy-secrets.sh

# Apply SecretManagerConfig
kubectl apply -f secret-manager-config.yaml

# Trigger immediate reconciliation
msmctl reconcile --name my-secrets --namespace default

# Wait for reconciliation to complete
kubectl wait --for=condition=Ready \
  secretmanagerconfig/my-secrets \
  --timeout=60s
```

#### Monitoring Script

Check status of all resources:

```bash
#!/bin/bash
# check-status.sh

for config in $(msmctl list --namespace pricewhisperer | awk 'NR>2 {print $1}'); do
  echo "Checking $config..."
  msmctl status --name "$config" --namespace pricewhisperer
  echo ""
done
```

## Related Components

- **Flux SourceController** - Provides GitRepository artifacts
- **GitHub SOPS Bot** - Manages GPG keys for secret encryption
- **Upbound GCP CloudRun Provider** - Consumes secrets from Secret Manager

