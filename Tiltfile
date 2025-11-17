# Secret Manager Controller Tiltfile
# 
# This Tiltfile matches PriceWhisperer's build pattern:
# 1. Builds Rust binaries on host (cross-compilation)
# 2. Copies binaries to build_artifacts/
# 3. Generates CRD using crdgen binary
# 4. Builds Docker image copying from build_artifacts/
# 5. Deploys to Kubernetes using kustomize
#
# Usage: tilt up

# ====================
# Configuration
# ====================

# Restrict to k3s cluster
allow_k8s_contexts(['k3s-secret-manager-controller'])

# Get the directory where this Tiltfile is located
# Since the Tiltfile is in the controller directory, use '.' for relative paths
CONTROLLER_DIR = '.'
CONTROLLER_NAME = 'secret-manager-controller'
IMAGE_NAME = 'localhost:5002/secret-manager-controller'
BINARY_NAME = 'secret-manager-controller'
# Build for Linux x86_64 (cross-compile for container compatibility)
BINARY_PATH = '%s/target/x86_64-unknown-linux-musl/debug/%s' % (CONTROLLER_DIR, BINARY_NAME)
CRDGEN_PATH = '%s/target/x86_64-unknown-linux-musl/debug/crdgen' % CONTROLLER_DIR
# Native crdgen for host execution (CRD generation runs on host, not in container)
CRDGEN_NATIVE_PATH = '%s/target/debug/crdgen' % CONTROLLER_DIR
ARTIFACT_PATH = 'build_artifacts/%s' % BINARY_NAME
CRDGEN_ARTIFACT_PATH = 'build_artifacts/crdgen'

# ====================
# Code Quality Checks
# ====================
# Run formatting and linting checks
# Disabled for now
# local_resource(
#     'secret-manager-controller-fmt-check',
#     cmd='''
#         echo "🎨 Checking code formatting..."
#         cargo fmt --all -- --check || {
#             echo "❌ Formatting check failed. Run 'cargo fmt' to fix."
#             exit 1
#         }
#         echo "✅ Formatting check passed"
#     ''',
#     deps=[
#         '%s/src' % CONTROLLER_DIR,
#         '%s/Cargo.toml' % CONTROLLER_DIR,
#     ],
#     resource_deps=[],
#     labels=['code-quality'],
#     allow_parallel=True,
# )

# local_resource(
#     'secret-manager-controller-clippy',
#     cmd='''
#         echo "🔍 Running clippy..."
#         cargo clippy --all-targets --all-features -- -D warnings || {
#             echo "❌ Clippy check failed. Fix the warnings above."
#             exit 1
#         }
#         echo "✅ Clippy check passed"
#     ''',
#     deps=[
#         '%s/src' % CONTROLLER_DIR,
#         '%s/Cargo.toml' % CONTROLLER_DIR,
#         '%s/Cargo.lock' % CONTROLLER_DIR,
#     ],
#     resource_deps=[],
#     labels=['code-quality'],
#     allow_parallel=True,
# )


# ====================
# Build and Copy Rust Binaries
# ====================
# Build binaries on host (cross-compilation) and copy to build_artifacts

local_resource(
    'secret-manager-controller-build-and-copy',
    cmd='python3 scripts/tilt/build_and_copy_binaries.py',
    deps=[
        '%s/src' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        '%s/Cargo.lock' % CONTROLLER_DIR,
        './scripts/host_aware_build.py',
        './scripts/copy_binary.py',
        './scripts/tilt/build_and_copy_binaries.py',
    ],
    env={
        'CONTROLLER_DIR': CONTROLLER_DIR,
        'BINARY_NAME': BINARY_NAME,
    },
    labels=['controllers'],
    allow_parallel=False,
)

# ====================
# CRD Generation
# ====================
# Generate CRD using crdgen binary from build_artifacts

local_resource(
    'secret-manager-controller-crd-gen',
    cmd='python3 scripts/tilt/generate_crd.py',
    deps=[
        CRDGEN_NATIVE_PATH,
        '%s/src' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        './scripts/tilt/generate_crd.py',
    ],
    env={
        'CONTROLLER_DIR': CONTROLLER_DIR,
    },
    resource_deps=['secret-manager-controller-build-and-copy'],
    labels=['controllers'],
    allow_parallel=True,
)

# ====================
# Docker Build
# ====================
# Build Docker image using custom_build (matches PriceWhisperer pattern)
# Note: docker_build.py handles image cleanup before building

custom_build(
    IMAGE_NAME,
    'python3 scripts/tilt/docker_build.py',
    deps=[
        ARTIFACT_PATH,
        CRDGEN_ARTIFACT_PATH,
        '%s/Dockerfile.dev' % CONTROLLER_DIR,
        './scripts/tilt/docker_build.py',
    ],
    env={
        'IMAGE_NAME': IMAGE_NAME,
        'CONTROLLER_NAME': CONTROLLER_NAME,
        'CONTROLLER_DIR': CONTROLLER_DIR,
    },
    tag='tilt',
    live_update=[
        sync(ARTIFACT_PATH, '/app/secret-manager-controller'),
        run('kill -HUP 1', trigger=[ARTIFACT_PATH]),
    ],
    skips_local_docker=False,
)

# ====================
# FluxCD Installation
# ====================
# Install FluxCD in the cluster before deploying the controller
# This ensures GitRepository CRDs and source-controller are available
# Idempotent - can be run multiple times safely

local_resource(
    'fluxcd-install',
    cmd='python3 scripts/tilt/install_fluxcd.py',
    deps=[
        './scripts/tilt/install_fluxcd.py',
    ],
    labels=['infrastructure'],
    allow_parallel=False,
)

# ====================
# Git Credentials Setup
# ====================
# Decrypt git credentials from SOPS-encrypted .env file and create Kubernetes secret
# This allows GitRepository resources to authenticate with private repositories
# Optional - only runs if .env file exists and contains git credentials

local_resource(
    'git-credentials-setup',
    cmd='python3 scripts/tilt/setup_git_credentials.py --all-environments',
    deps=[
        './scripts/tilt/setup_git_credentials.py',
        '.env',  # Watch for .env file changes
    ],
    labels=['infrastructure'],
    resource_deps=['fluxcd-install'],
    allow_parallel=False,
)

# ====================
# Deploy to Kubernetes
# ====================
# Deploy using kustomize
# Note: CRD file must exist before kustomize runs (generated by crd-gen resource)
# Note: FluxCD should be installed first (fluxcd-install resource)

k8s_yaml(kustomize('%s/config' % CONTROLLER_DIR))

# Configure resource
# Tilt will automatically substitute the image in the deployment
# because custom_build registers the image and Tilt matches it to the deployment
# Note: No port forwarding needed - pods get their own IPs
# Use 'kubectl port-forward' or 'just port-forward' to access metrics
k8s_resource(
    CONTROLLER_NAME,
    labels=['controllers'],
    resource_deps=['secret-manager-controller-build-and-copy', 'secret-manager-controller-crd-gen', 'fluxcd-install'],
)

# ====================
# Pact Broker Deployment
# ====================
# Deploy Pact Broker for contract testing
# 
# INDEPENDENT: Pact resources operate independently of controller resources.
# They can be started/stopped/managed separately using Tilt labels.
# Use 'tilt up pact' to run only Pact resources, or filter by label in UI.
# Note: Labels on k8s_resource() ensure isolation - controller won't wait for pact resources.

k8s_yaml(kustomize('pact-broker/k8s'))

k8s_resource(
    'pact-broker',
    labels=['pact'],
    port_forwards=['9292:9292'],
    # No resource_deps - completely independent from controllers
)

# ====================
# Pact Contract Publishing
# ====================
# Run Pact tests and publish contracts to broker
# 
# INDEPENDENT: Only depends on pact-broker, not on controller resources.
# Can run independently: 'tilt up pact' or filter by 'pact' label.

local_resource(
    'pact-tests-and-publish',
    cmd='python3 scripts/pact_publish.py',
    deps=[
        '%s/tests' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        'scripts/pact_publish.py',
    ],
    resource_deps=['pact-broker'],  # Only depends on pact-broker, not controllers
    labels=['pact'],
    allow_parallel=False,
)

# ====================
# GitOps Activation
# ====================
# Activate GitOps resources for tilt environment
# Applies GitRepository and other GitOps resources, triggering reconciliation
# Applied after git-credentials are set up to ensure secret exists if GitRepository references it

local_resource(
    'gitops-activate',
    cmd='kubectl apply -k gitops/cluster/env/tilt',
    deps=[
        'gitops/cluster/env/tilt/namespace.yaml',
        'gitops/cluster/env/tilt/gitrepository.yaml',
        'gitops/cluster/env/tilt/secretmanagerconfig.yaml',
        'gitops/cluster/env/tilt/kustomization.yaml',
    ],
    labels=['infrastructure'],
    resource_deps=['git-credentials-setup'],
    allow_parallel=False,
)

# Also load via k8s_yaml for Tilt to track the resource
k8s_yaml(kustomize('gitops/cluster/env/tilt'))

# ====================
# Test Resource Management
# ====================
# Install/update CRD (if changed) and apply test SecretManagerConfig resource
# Independent resource - can be run separately for testing
# Note: CRD is applied (not deleted) - kubectl apply handles install/update automatically

local_resource(
    'test-resource-update',
    cmd='python3 scripts/tilt/reset_test_resource.py',
    deps=[
        'gitops/cluster/env/tilt/secretmanagerconfig.yaml',
        'gitops/cluster/env/stage/secretmanagerconfig.yaml',
        'gitops/cluster/env/prod/secretmanagerconfig.yaml',
        'config/crd/secretmanagerconfig.yaml',
        './scripts/tilt/reset_test_resource.py',
    ],
    env={
        'CONTROLLER_DIR': CONTROLLER_DIR,
    },
    resource_deps=['secret-manager-controller-crd-gen'],
    labels=['test'],
    allow_parallel=True,
)
