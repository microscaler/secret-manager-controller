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
# Build Rust Binaries
# ====================
# Build both controller and crdgen binaries on host (cross-compilation)

local_resource(
    'secret-manager-controller-build',
    cmd='''
        # Build Linux binaries for container (cross-compilation)
        ./scripts/host-aware-build.sh --bin %s --bin crdgen
        # Also build native crdgen for host execution (CRD generation)
        cargo build --bin crdgen
    ''' % BINARY_NAME,
    deps=[
        '%s/src' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        '%s/Cargo.lock' % CONTROLLER_DIR,
        './scripts/host-aware-build.sh',
    ],
    labels=['controllers'],
    allow_parallel=False,
)

# ====================
# Copy Binaries to Artifacts
# ====================
# Copy binaries to build_artifacts directory for Docker builds

local_resource(
    'secret-manager-controller-copy',
    cmd='''
        ./scripts/copy-binary.sh %s %s %s
        ./scripts/copy-binary.sh %s %s crdgen
    ''' % (BINARY_PATH, ARTIFACT_PATH, BINARY_NAME, CRDGEN_PATH, CRDGEN_ARTIFACT_PATH),
    deps=[BINARY_PATH, CRDGEN_PATH, './scripts/copy-binary.sh'],
    resource_deps=['secret-manager-controller-build'],
    labels=['controllers'],
    allow_parallel=False,
)

# ====================
# CRD Generation
# ====================
# Generate CRD using crdgen binary from build_artifacts

local_resource(
    'secret-manager-controller-crd-gen',
    cmd='''
        mkdir -p config/crd
        # Check if native crdgen binary exists
        if [ ! -f "%s" ]; then
            echo "❌ Error: crdgen binary not found at %s" >&2
            echo "   Make sure 'secret-manager-controller-build' has completed" >&2
            exit 1
        fi
        # Use native crdgen binary (runs on host, not in container)
        # Redirect stdout to CRD file, stderr to Tilt logs separately
        # This ensures error messages don't corrupt the CRD file
        RUST_LOG=off "%s" > config/crd/secretmanagerconfig.yaml 2> /tmp/crdgen-stderr.log
        exit_code=$?
        if [ $exit_code -ne 0 ]; then
            echo "❌ Error: CRD generation command failed with exit code $exit_code" >&2
            if [ -s /tmp/crdgen-stderr.log ]; then
                echo "Error output:" >&2
                cat /tmp/crdgen-stderr.log >&2
            fi
            # Don't leave invalid YAML in the CRD file
            rm -f config/crd/secretmanagerconfig.yaml
            exit $exit_code
        fi
        # Validate CRD is valid YAML (must contain apiVersion, kind, or --- after comments)
        # Skip comment lines and check for actual YAML content
        if ! grep -v '^#' config/crd/secretmanagerconfig.yaml | grep -qE '^(apiVersion|kind|---)'; then
            echo "❌ Error: CRD generation failed - file does not contain valid YAML" >&2
            echo "First 10 lines of output:" >&2
            head -10 config/crd/secretmanagerconfig.yaml >&2
            exit 1
        fi
        echo "✅ CRD generated successfully"
    ''' % (CRDGEN_NATIVE_PATH, CRDGEN_NATIVE_PATH, CRDGEN_NATIVE_PATH),
    deps=[
        CRDGEN_NATIVE_PATH,
        '%s/src' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
    ],
    resource_deps=['secret-manager-controller-build'],
    labels=['controllers'],
    allow_parallel=True,
)

# ====================
# Docker Build
# ====================
# Build Docker image using custom_build (matches PriceWhisperer pattern)

custom_build(
    IMAGE_NAME,
    'docker build -f %s/Dockerfile.dev -t %s:tilt %s && docker tag %s:tilt $EXPECTED_REF && docker push $EXPECTED_REF' % (
        CONTROLLER_DIR,
        IMAGE_NAME,
        CONTROLLER_DIR,
        IMAGE_NAME
    ),
    deps=[
        ARTIFACT_PATH,
        CRDGEN_ARTIFACT_PATH,
        '%s/Dockerfile.dev' % CONTROLLER_DIR,
    ],
    tag='tilt',
    live_update=[
        sync(ARTIFACT_PATH, '/app/secret-manager-controller'),
        run('kill -HUP 1', trigger=[ARTIFACT_PATH]),
    ],
)

# ====================
# Deploy to Kubernetes
# ====================
# Deploy using kustomize
# Note: CRD file must exist before kustomize runs (generated by crd-gen resource)

k8s_yaml(kustomize('%s/config' % CONTROLLER_DIR))

# Configure resource
# Tilt will automatically substitute the image in the deployment
# because custom_build registers the image and Tilt matches it to the deployment
# Note: No port forwarding needed - pods get their own IPs
# Use 'kubectl port-forward' or 'just port-forward' to access metrics
k8s_resource(
    CONTROLLER_NAME,
    labels=['controllers'],
    resource_deps=['secret-manager-controller-copy', 'secret-manager-controller-crd-gen'],
)

# ====================
# Pact Broker Deployment
# ====================
# Deploy Pact Broker for contract testing

k8s_yaml(kustomize('pact-broker/k8s'))

k8s_resource(
    'pact-broker',
    labels=['pact'],
    port_forwards=['9292:9292'],
)

# ====================
# Pact Contract Publishing
# ====================
# Run Pact tests and publish contracts to broker

local_resource(
    'pact-tests-and-publish',
    cmd='python3 scripts/pact_publish.py',
    deps=[
        '%s/tests' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        'scripts/pact_publish.py',
    ],
    resource_deps=['pact-broker'],
    labels=['pact'],
    allow_parallel=False,
)
