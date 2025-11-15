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
        # Validate CRD is valid YAML (must start with apiVersion, kind, or ---)
        if ! head -1 config/crd/secretmanagerconfig.yaml | grep -qE '^(apiVersion|kind|---)'; then
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
    cmd='''
        # Wait for Pact broker to be ready
        echo "⏳ Waiting for Pact broker to be ready..."
        kubectl wait --for=condition=ready pod -l app=pact-broker -n secret-manager-controller-pact-broker --timeout=120s || {
            echo "❌ Pact broker not ready"
            exit 1
        }
        
        # Set up port forwarding in background
        echo "🔌 Setting up port forwarding to Pact broker..."
        kubectl port-forward -n secret-manager-controller-pact-broker service/pact-broker 9292:9292 > /tmp/pact-port-forward.log 2>&1 &
        PORT_FORWARD_PID=$!
        sleep 3
        
        # Check if port forward is working
        if ! curl -s -u pact:pact http://localhost:9292 > /dev/null 2>&1; then
            echo "❌ Failed to connect to Pact broker"
            kill $PORT_FORWARD_PID 2>/dev/null || true
            exit 1
        fi
        
        echo "✅ Pact broker is ready"
        
        # Run Pact tests (this generates pact files)
        echo "🧪 Running Pact contract tests..."
        
        # Run tests and capture output
        cargo test --test pact_* --no-fail-fast 2>&1 | tee /tmp/pact-tests.log
        TEST_EXIT_CODE=${PIPESTATUS[0]}
        
        # Find and publish Pact files using Pact CLI
        PACT_DIR="target/pacts"
        if [ -d "$PACT_DIR" ] && [ "$(ls -A $PACT_DIR/*.json 2>/dev/null)" ]; then
            echo "📦 Publishing Pact contracts to broker..."
            
            # Get git info for versioning
            GIT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "main")
            GIT_COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "dev")
            VERSION="${GIT_BRANCH}-${GIT_COMMIT}"
            
            # Check if pact-broker CLI is available, if not use Docker
            PUBLISH_EXIT_CODE=0
            if command -v pact-broker &> /dev/null; then
                # Use local Pact CLI
                echo "  Using local Pact CLI..."
                if ! pact-broker publish "$PACT_DIR" \
                    --consumer-app-version "$VERSION" \
                    --branch "$GIT_BRANCH" \
                    --broker-base-url http://localhost:9292 \
                    --broker-username pact \
                    --broker-password pact; then
                    PUBLISH_EXIT_CODE=$?
                fi
            else
                # Use Docker image for Pact CLI
                # Note: Use host.docker.internal to access localhost from Docker container
                # On Linux, we may need --network host instead
                echo "  Using Pact CLI Docker image..."
                if ! docker run --rm \
                    --add-host=host.docker.internal:host-gateway \
                    -v "$(pwd):/pacts" \
                    -w /pacts \
                    pactfoundation/pact-cli:latest \
                    publish "$PACT_DIR" \
                    --consumer-app-version "$VERSION" \
                    --branch "$GIT_BRANCH" \
                    --broker-base-url http://host.docker.internal:9292 \
                    --broker-username pact \
                    --broker-password pact; then
                    PUBLISH_EXIT_CODE=$?
                fi
            fi
            
            if [ $PUBLISH_EXIT_CODE -eq 0 ]; then
                echo "✅ Published Pact contracts to broker"
                echo "   Version: $VERSION"
                echo "   Branch: $GIT_BRANCH"
                echo "   View at: http://localhost:9292"
            else
                echo "⚠️  Failed to publish contracts (exit code: $PUBLISH_EXIT_CODE)"
                echo "   Check that port forwarding is active and broker is accessible"
            fi
        else
            echo "ℹ️  No Pact files found in $PACT_DIR"
            echo "   Pact files are generated when tests run successfully"
        fi
        
        # Clean up port forward
        kill $PORT_FORWARD_PID 2>/dev/null || true
        
        if [ $TEST_EXIT_CODE -eq 0 ]; then
            echo "✅ Pact tests passed"
        else
            echo "⚠️  Some Pact tests failed (exit code: $TEST_EXIT_CODE)"
        fi
        
        exit $TEST_EXIT_CODE
    ''',
    deps=[
        '%s/tests' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
    ],
    resource_deps=['pact-broker'],
    labels=['pact'],
    allow_parallel=False,
)
