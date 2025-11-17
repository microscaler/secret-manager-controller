#!/usr/bin/env python3
"""
Install ArgoCD in Kubernetes cluster.

This script ensures ArgoCD components (application-controller, Application CRD) are installed
before deploying the secret-manager-controller, which can use ArgoCD Applications as sources.
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


def check_argocd_installed():
    """Check if ArgoCD is already installed in the cluster."""
    # Check if argocd namespace exists
    ns_result = run_command(
        "kubectl get namespace argocd",
        check=False,
        capture_output=True
    )
    
    if ns_result.returncode != 0:
        return False
    
    # Check if namespace is terminating
    phase_result = run_command(
        "kubectl get namespace argocd -o jsonpath='{.status.phase}'",
        check=False,
        capture_output=True
    )
    
    if phase_result.returncode == 0 and "Terminating" in phase_result.stdout:
        log_warn("⚠️  argocd namespace is terminating")
        return False
    
    # Check if application-controller pod is running
    pod_result = run_command(
        "kubectl get pods -n argocd -l app.kubernetes.io/name=argocd-application-controller --field-selector=status.phase=Running -o name",
        check=False,
        capture_output=True
    )
    
    if pod_result.returncode == 0 and "argocd-application-controller" in pod_result.stdout:
        log_info("✅ ArgoCD is already installed (application-controller running)")
        return True
    
    # Check if Application CRD exists
    crd_result = run_command(
        "kubectl get crd applications.argoproj.io",
        check=False,
        capture_output=True
    )
    
    if crd_result.returncode == 0:
        log_info("✅ ArgoCD CRDs are installed")
        return True
    
    return False


def install_argocd():
    """Install ArgoCD using kubectl apply."""
    log_info("Installing ArgoCD...")
    
    # Install ArgoCD using the official installation manifest
    # This installs ArgoCD in the argocd namespace
    log_info("Applying ArgoCD installation manifest...")
    
    # Use the official ArgoCD installation manifest
    # Version 2.10+ supports installation via kubectl apply
    install_cmd = (
        "kubectl create namespace argocd --dry-run=client -o yaml | kubectl apply -f - && "
        "kubectl apply -n argocd -f https://raw.githubusercontent.com/argoproj/argo-cd/stable/manifests/install.yaml"
    )
    
    result = run_command(
        install_cmd,
        check=False,
        capture_output=True
    )
    
    if result.returncode != 0:
        # Check if it failed because resources already exist
        if "already exists" in result.stderr.lower() or "AlreadyExists" in result.stderr:
            log_info("ArgoCD resources already exist")
        else:
            log_error(f"Failed to install ArgoCD: {result.stderr}")
            return False
    
    log_info("ArgoCD installation manifest applied")
    log_info("Waiting for ArgoCD components to be ready...")
    
    # Wait for application-controller to be ready
    max_attempts = 60  # Wait up to 2 minutes
    for i in range(max_attempts):
        result = run_command(
            "kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=argocd-application-controller -n argocd --timeout=10s",
            check=False,
            capture_output=True
        )
        
        if result.returncode == 0:
            log_info("✅ ArgoCD application-controller is ready!")
            break
        
        if i < max_attempts - 1:
            log_info(f"Waiting for application-controller... ({i+1}/{max_attempts})")
            time.sleep(2)
        else:
            log_warn("Application-controller not ready after 2 minutes, but installation may have succeeded")
    
    # Check other components
    components = [
        ("argocd-server", "app.kubernetes.io/name=argocd-server"),
        ("argocd-repo-server", "app.kubernetes.io/name=argocd-repo-server"),
        ("argocd-redis", "app.kubernetes.io/name=argocd-redis"),
    ]
    
    for component_name, label_selector in components:
        result = run_command(
            f"kubectl get pods -n argocd -l {label_selector} --field-selector=status.phase=Running",
            check=False,
            capture_output=True
        )
        if result.returncode == 0 and component_name in result.stdout:
            log_info(f"✅ {component_name} is running")
        else:
            log_warn(f"⚠️  {component_name} not ready yet (may still be starting)")
    
    # Verify Application CRD exists
    result = run_command(
        "kubectl get crd applications.argoproj.io",
        check=False,
        capture_output=True
    )
    
    if result.returncode == 0:
        log_info("✅ Application CRD is installed")
    else:
        log_warn("⚠️  Application CRD not found - this may cause issues")
    
    return True


def main():
    """Main function."""
    log_info("ArgoCD Installation Script")
    log_info("=" * 50)
    
    # Check if already installed
    is_installed = check_argocd_installed()
    
    # If namespace is terminating, wait for cleanup before proceeding
    ns_result = run_command(
        "kubectl get namespace argocd -o jsonpath='{.status.phase}'",
        check=False,
        capture_output=True
    )
    
    if ns_result.returncode == 0 and "Terminating" in ns_result.stdout:
        log_warn("⚠️  argocd namespace is currently terminating")
        log_warn("   This was likely triggered by a previous deletion or failed installation")
        log_warn("   The script is NOT deleting it - waiting for existing deletion to complete...")
        log_info("   This may take a few minutes. Please wait...")
        
        # Wait for namespace to be fully deleted
        max_wait = 300  # Wait up to 5 minutes
        finalizer_clear_attempted = False
        
        for i in range(max_wait):
            check_result = run_command(
                "kubectl get namespace argocd",
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
                if clear_namespace_finalizers("argocd"):
                    finalizer_clear_attempted = True
                    log_info("   Waiting for namespace deletion to complete after clearing finalizers...")
            
            if i % 30 == 0 and i > 0:
                log_info(f"   Still waiting... ({i}/{max_wait}s)")
            time.sleep(1)
        else:
            log_error("Timeout waiting for namespace cleanup (5 minutes)")
            log_error("The namespace deletion is stuck even after clearing finalizers.")
            log_error("You can try force-deleting it:")
            log_error("  kubectl delete namespace argocd --force --grace-period=0")
            log_error("Then re-run this script.")
            sys.exit(1)
    
    if is_installed:
        log_info("")
        log_info("✅ ArgoCD installation check complete!")
        return
    
    # Install ArgoCD
    if not install_argocd():
        sys.exit(1)
    
    log_info("")
    log_info("✅ ArgoCD installation complete!")
    log_info("📋 Next steps:")
    log_info("  1. Create Application resources in your environment namespaces")
    log_info("  2. Create SecretManagerConfig resources that reference them")
    log_info("  3. Verify Applications are created: kubectl get application -A")
    log_info("")
    log_info("💡 Note: ArgoCD Applications can be created in any namespace")
    log_info("   The secret-manager-controller will clone repositories from Application specs")


if __name__ == "__main__":
    main()

