#!/usr/bin/env python3
"""
Install FluxCD in Kubernetes cluster.

This script ensures FluxCD components (source-controller, GitRepository CRD) are installed
before deploying the secret-manager-controller, which depends on them.
"""

import subprocess
import sys
import time


def run_command(cmd, check=True, capture_output=True):
    """Run a shell command and return the result."""
    result = subprocess.run(
        cmd,
        shell=True,
        capture_output=capture_output,
        text=True
    )
    if check and result.returncode != 0:
        print(f"Error: Command failed: {cmd}", file=sys.stderr)
        if result.stderr:
            print(result.stderr, file=sys.stderr)
        sys.exit(1)
    return result


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_warn(msg):
    """Print warning message."""
    print(f"[WARN] {msg}", file=sys.stderr)


def log_error(msg):
    """Print error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def clear_namespace_finalizers(namespace):
    """Clear finalizers from a namespace to allow deletion to proceed."""
    log_info(f"🔧 Attempting to clear finalizers for namespace '{namespace}'...")
    
    # Patch the namespace to remove all finalizers
    patch_cmd = f"kubectl patch namespace {namespace} -p '{{\"metadata\":{{\"finalizers\":[]}}}}' --type=merge"
    result = run_command(patch_cmd, check=False, capture_output=True)
    
    if result.returncode == 0:
        log_info(f"✅ Successfully cleared finalizers for namespace '{namespace}'")
        return True
    else:
        log_warn(f"⚠️  Failed to clear finalizers: {result.stderr}")
        return False


def check_flux_cli():
    """Check if flux CLI is installed."""
    result = run_command("which flux", check=False, capture_output=True)
    if result.returncode != 0:
        log_error("flux CLI not found. Please install it:")
        log_error("  brew install fluxcd/tap/flux  # macOS")
        log_error("  or see: https://fluxcd.io/docs/installation/")
        return False
    return True


def check_fluxcd_installed():
    """Check if FluxCD is already installed in the cluster using flux check."""
    # First check if namespace is terminating - if so, FluxCD is not properly installed
    ns_result = run_command(
        "kubectl get namespace flux-system -o jsonpath='{.status.phase}'",
        check=False,
        capture_output=True
    )
    
    if ns_result.returncode == 0 and "Terminating" in ns_result.stdout:
        log_warn("⚠️  flux-system namespace is terminating")
        log_warn("   FluxCD is not properly installed - namespace is being deleted")
        return False
    
    # Use flux check command to verify installation
    result = run_command(
        "flux check",
        check=False,
        capture_output=True
    )
    
    # flux check returns non-zero if checks fail, but we need to check if controllers exist
    # Check for controllers in the output
    if "controllers" in result.stdout.lower():
        # Check if source-controller exists and is running
        pod_result = run_command(
            "kubectl get pods -n flux-system -l app.kubernetes.io/name=source-controller --field-selector=status.phase=Running -o name",
            check=False,
            capture_output=True
        )
        if pod_result.returncode == 0 and "source-controller" in pod_result.stdout:
            log_info("✅ FluxCD is already installed (source-controller running)")
            return True
    
    # Fallback: Check namespace and pods if flux check doesn't show controllers
    ns_check = run_command(
        "kubectl get namespace flux-system",
        check=False,
        capture_output=True
    )
    if ns_check.returncode == 0:
        # Check if FluxCD components are running
        pod_check = run_command(
            "kubectl get pods -n flux-system -l app.kubernetes.io/name=source-controller --field-selector=status.phase=Running",
            check=False,
            capture_output=True
        )
        if pod_check.returncode == 0 and "source-controller" in pod_check.stdout:
            log_info("✅ FluxCD is already installed (source-controller running)")
            return True
    
    return False


def install_fluxcd():
    """Install FluxCD using Flux CLI bootstrap."""
    log_info("Installing FluxCD...")
    
    # Use bootstrap command with:
    # - flux-system namespace (standard)
    # - No Git repository (we'll create GitRepositories manually for testing)
    # - No GitOps mode (just install components)
    log_info("Running: flux install --namespace=flux-system")
    
    result = run_command(
        "flux install --namespace=flux-system",
        check=False,
        capture_output=True
    )
    
    if result.returncode != 0:
        # Check if it failed because FluxCD is already installed
        if "already installed" in result.stderr.lower() or "already exists" in result.stderr.lower():
            log_info("FluxCD appears to be already installed (installation command detected existing installation)")
            return True
        log_error(f"Failed to install FluxCD: {result.stderr}")
        return False
    
    log_info("FluxCD installation command completed")
    log_info("Waiting for FluxCD components to be ready...")
    
    # Wait for source-controller to be ready
    max_attempts = 30
    for i in range(max_attempts):
        result = run_command(
            "kubectl wait --for=condition=ready pod -l app=source-controller -n flux-system --timeout=10s",
            check=False,
            capture_output=True
        )
        
        if result.returncode == 0:
            log_info("✅ FluxCD source-controller is ready!")
            break
        
        if i < max_attempts - 1:
            log_info(f"Waiting for source-controller... ({i+1}/{max_attempts})")
            time.sleep(2)
        else:
            log_warn("Source-controller not ready after 60 seconds, but installation may have succeeded")
    
    # Configure source-controller to watch all namespaces
    # This allows GitRepositories in tilt, dev, stage, prod namespaces to be processed
    log_info("Configuring source-controller to watch all namespaces...")
    
    # Check if --watch-all-namespaces flag already exists
    result = run_command(
        "kubectl get deployment source-controller -n flux-system -o jsonpath='{.spec.template.spec.containers[0].args}'",
        check=False,
        capture_output=True
    )
    
    if result.returncode == 0 and "--watch-all-namespaces=true" not in result.stdout:
        # Patch the deployment to add --watch-all-namespaces flag
        patch_result = run_command(
            "kubectl patch deployment source-controller -n flux-system --type='json' -p='[{\"op\": \"add\", \"path\": \"/spec/template/spec/containers/0/args/-\", \"value\": \"--watch-all-namespaces=true\"}]'",
            check=False,
            capture_output=True
        )
        
        if patch_result.returncode == 0:
            log_info("✅ Configured source-controller to watch all namespaces")
            log_info("Waiting for source-controller to restart with new configuration...")
            time.sleep(5)
            
            # Wait for the new pod to be ready
            for i in range(30):
                result = run_command(
                    "kubectl wait --for=condition=ready pod -l app=source-controller -n flux-system --timeout=10s",
                    check=False,
                    capture_output=True
                )
                if result.returncode == 0:
                    log_info("✅ source-controller restarted and ready with multi-namespace support")
                    break
                time.sleep(2)
        else:
            log_warn(f"⚠️  Failed to configure source-controller for multi-namespace: {patch_result.stderr}")
            log_warn("GitRepositories in non-flux-system namespaces may not be processed")
            log_warn("See gitops/cluster/fluxcd/FLUXCD_MULTI_NAMESPACE.md for manual configuration")
    else:
        if result.returncode == 0 and "--watch-all-namespaces=true" in result.stdout:
            log_info("✅ source-controller already configured to watch all namespaces")
        else:
            log_warn("⚠️  Could not verify source-controller configuration")
    
    # Check other components
    components = [
        ("kustomize-controller", "app=kustomize-controller"),
        ("helm-controller", "app=helm-controller"),
        ("notification-controller", "app=notification-controller"),
    ]
    
    for component_name, label_selector in components:
        result = run_command(
            f"kubectl get pods -n flux-system -l {label_selector}",
            check=False,
            capture_output=True
        )
        if result.returncode == 0 and component_name in result.stdout:
            log_info(f"✅ {component_name} is running")
        else:
            log_warn(f"⚠️  {component_name} not found (optional component)")
    
    # Verify GitRepository CRD exists
    result = run_command(
        "kubectl get crd gitrepositories.source.toolkit.fluxcd.io",
        check=False,
        capture_output=True
    )
    
    if result.returncode == 0:
        log_info("✅ GitRepository CRD is installed")
    else:
        log_warn("⚠️  GitRepository CRD not found - this may cause issues")
    
    return True


def main():
    """Main function."""
    log_info("FluxCD Installation Script")
    log_info("=" * 50)
    
    # Check prerequisites
    if not check_flux_cli():
        sys.exit(1)
    
    # Check if already installed
    is_installed = check_fluxcd_installed()
    
    # If namespace is terminating, wait for cleanup before proceeding
    # Note: The script is NOT deleting the namespace - it's detecting that something else
    # (previous deletion, failed installation, etc.) has already triggered deletion
    ns_result = run_command(
        "kubectl get namespace flux-system -o jsonpath='{.status.phase}'",
        check=False,
        capture_output=True
    )
    
    if ns_result.returncode == 0 and "Terminating" in ns_result.stdout:
        log_warn("⚠️  flux-system namespace is currently terminating")
        log_warn("   This was likely triggered by a previous deletion or failed installation")
        log_warn("   The script is NOT deleting it - waiting for existing deletion to complete...")
        log_info("   This may take a few minutes. Please wait...")
        
        # Wait for namespace to be fully deleted (longer timeout for namespace deletion)
        max_wait = 300  # Wait up to 5 minutes
        finalizer_clear_attempted = False
        
        for i in range(max_wait):
            check_result = run_command(
                "kubectl get namespace flux-system",
                check=False,
                capture_output=True
            )
            if check_result.returncode != 0:
                log_info("✅ Namespace cleanup complete")
                is_installed = False  # Reset since namespace was deleted
                break
            
            # If namespace is still terminating after 60 seconds, try clearing finalizers
            if i == 60 and not finalizer_clear_attempted:
                log_warn("⚠️  Namespace still terminating after 60 seconds")
                log_info("   Attempting to clear finalizers to allow deletion...")
                if clear_namespace_finalizers("flux-system"):
                    finalizer_clear_attempted = True
                    log_info("   Waiting for namespace deletion to complete after clearing finalizers...")
            
            if i % 30 == 0 and i > 0:
                log_info(f"   Still waiting... ({i}/{max_wait}s)")
            time.sleep(1)
        else:
            log_error("Timeout waiting for namespace cleanup (5 minutes)")
            log_error("The namespace deletion is stuck even after clearing finalizers.")
            log_error("You can try force-deleting it:")
            log_error("  kubectl delete namespace flux-system --force --grace-period=0")
            log_error("Then re-run this script.")
            sys.exit(1)
    
    if is_installed:
        log_info("FluxCD is already installed. Verifying configuration...")
        # Still configure multi-namespace support if needed
        result = run_command(
            "kubectl get deployment source-controller -n flux-system -o jsonpath='{.spec.template.spec.containers[0].args}'",
            check=False,
            capture_output=True
        )
        if result.returncode == 0 and "--watch-all-namespaces=true" not in result.stdout:
            log_info("Configuring source-controller for multi-namespace support...")
            patch_result = run_command(
                "kubectl patch deployment source-controller -n flux-system --type='json' -p='[{\"op\": \"add\", \"path\": \"/spec/template/spec/containers/0/args/-\", \"value\": \"--watch-all-namespaces=true\"}]'",
                check=False,
                capture_output=True
            )
            if patch_result.returncode == 0:
                log_info("✅ Configured source-controller to watch all namespaces")
            else:
                log_warn(f"⚠️  Failed to configure: {patch_result.stderr}")
        else:
            if result.returncode == 0 and "--watch-all-namespaces=true" in result.stdout:
                log_info("✅ source-controller already configured for multi-namespace")
        log_info("")
        log_info("✅ FluxCD installation check complete!")
        return
    
    # Install FluxCD
    if not install_fluxcd():
        sys.exit(1)
    
    log_info("")
    log_info("✅ FluxCD installation complete!")
    log_info("📋 Next steps:")
    log_info("  1. Create GitRepository resources in your environment namespaces")
    log_info("  2. Create SecretManagerConfig resources that reference them")
    log_info("  3. Verify GitRepositories are processed: kubectl get gitrepository -A")


if __name__ == "__main__":
    main()
