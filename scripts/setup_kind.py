#!/usr/bin/env python3
"""
Secret Manager Controller Kind Cluster Setup Script.

This script replaces setup-kind.sh and provides better error handling
and cross-platform support.

Creates a local Kind cluster with Docker registry for development.
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path


# Configuration
CLUSTER_NAME = "secret-manager-controller"
REGISTRY_NAME = "secret-manager-controller-registry"
REGISTRY_PORT = "5002"


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_warn(msg):
    """Print warning message."""
    print(f"[WARN] {msg}")


def log_error(msg):
    """Print error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def check_command(cmd):
    """Check if a command exists."""
    if not shutil.which(cmd):
        log_error(f"{cmd} is not installed. Please install it first.")
        sys.exit(1)


def run_command(cmd, check=True, capture_output=True, **kwargs):
    """Run a command and return the result."""
    result = subprocess.run(
        cmd,
        shell=isinstance(cmd, str),
        capture_output=capture_output,
        text=True,
        check=check,
        **kwargs
    )
    return result


def setup_registry():
    """Setup local Docker registry."""
    result = run_command(f"docker ps --format '{{{{.Names}}}}'", check=False)
    
    if REGISTRY_NAME not in result.stdout:
        log_info("Creating local Docker registry...")
        run_command(
            f"docker run -d --restart=always -p {REGISTRY_PORT}:5000 --name {REGISTRY_NAME} registry:2"
        )
    else:
        log_info("Local registry already running")


def setup_kind_cluster():
    """Setup Kind cluster."""
    result = run_command("kind get clusters", check=False)
    
    if CLUSTER_NAME in result.stdout:
        log_warn(f"Cluster {CLUSTER_NAME} already exists")
        response = input("Do you want to delete and recreate it? (y/N) ")
        if response.lower() == 'y':
            log_info("Deleting existing cluster...")
            run_command(f"kind delete cluster --name {CLUSTER_NAME}")
        else:
            log_info("Using existing cluster")
            sys.exit(0)
    
    # Check if kind-config.yaml exists
    config_path = Path("kind-config.yaml")
    if not config_path.exists():
        log_error(f"kind-config.yaml not found at {config_path}")
        log_info("Please create kind-config.yaml in the project root")
        sys.exit(1)
    
    log_info("Creating Kind cluster...")
    run_command(f"kind create cluster --config {config_path}")
    
    # Connect registry to cluster network
    result = run_command("docker network ls --format '{{{{.Name}}}}'", check=False)
    if "kind" in result.stdout:
        run_command(f"docker network connect kind {REGISTRY_NAME}", check=False)
    
    # Configure cluster to use local registry
    configmap_yaml = f"""apiVersion: v1
kind: ConfigMap
metadata:
  name: local-registry-hosting
  namespace: kube-public
data:
  localRegistryHosting.v1: |
    host: "localhost:{REGISTRY_PORT}"
    help: "https://kind.sigs.k8s.io/docs/user/local-registry/"
"""
    
    run_command(
        "kubectl apply -f -",
        input=configmap_yaml,
        check=True
    )
    
    log_info(f"✅ Kind cluster '{CLUSTER_NAME}' created successfully!")
    log_info(f"📦 Local registry: {REGISTRY_NAME} (localhost:{REGISTRY_PORT})")
    log_info("🚀 You can now run 'tilt up' to start the controller")


def main():
    """Main setup function."""
    log_info("Checking prerequisites...")
    check_command("docker")
    check_command("kind")
    check_command("kubectl")
    
    setup_registry()
    setup_kind_cluster()


if __name__ == "__main__":
    main()

