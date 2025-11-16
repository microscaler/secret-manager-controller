#!/usr/bin/env python3
"""
Extract and merge k3s kubeconfig.

This script extracts the kubeconfig from the k3s container and merges it
into the main kubeconfig file.
"""

import subprocess
import sys
import shutil
from pathlib import Path

CONTAINER_NAME = "k3s-secret-manager-controller"
CLUSTER_NAME = "secret-manager-controller"


def run_command(cmd, check=True, capture_output=True):
    """Run a command and return the result."""
    result = subprocess.run(
        cmd,
        shell=True,
        check=check,
        capture_output=capture_output,
        text=True
    )
    return result


def main():
    """Main function."""
    print("🔧 Extracting k3s kubeconfig...")
    
    # Check if k3s container exists
    result = run_command(f"docker ps --format '{{{{.Names}}}}' | grep -q ^{CONTAINER_NAME}$", check=False)
    if result.returncode != 0:
        print(f"❌ Error: K3s container '{CONTAINER_NAME}' not found or not running")
        sys.exit(1)
    
    # Check if kubeconfig exists in container
    result = run_command(f"docker exec {CONTAINER_NAME} test -f /etc/rancher/k3s/k3s.yaml", check=False)
    if result.returncode != 0:
        print("❌ Error: k3s.yaml not found in container. Is k3s fully started?")
        sys.exit(1)
    
    # Get kubeconfig
    print("📋 Copying kubeconfig from container...")
    kube_dir = Path.home() / ".kube"
    kube_dir.mkdir(parents=True, exist_ok=True)
    
    kubeconfig_path = kube_dir / f"k3s-{CLUSTER_NAME}.yaml"
    result = run_command(
        f"docker cp {CONTAINER_NAME}:/etc/rancher/k3s/k3s.yaml {kubeconfig_path}",
        check=True
    )
    
    # Update kubeconfig to use localhost instead of 127.0.0.1
    print("🔧 Updating kubeconfig to use localhost...")
    if kubeconfig_path.exists():
        content = kubeconfig_path.read_text()
        content = content.replace("127.0.0.1", "localhost")
        kubeconfig_path.write_text(content)
        print(f"✅ Kubeconfig saved to: {kubeconfig_path}")
    else:
        print(f"❌ Error: Failed to copy kubeconfig")
        sys.exit(1)
    
    # Merge kubeconfig into main config
    print("🔀 Merging kubeconfig into main config...")
    main_config = kube_dir / "config"
    if main_config.exists():
        print(f"📝 Merging with existing config: {main_config}")
        result = run_command(
            f"KUBECONFIG={kubeconfig_path}:{main_config} kubectl config view --flatten > {main_config}.new",
            check=False
        )
        if result.returncode == 0 and (main_config.with_suffix(".new")).exists():
            (main_config.with_suffix(".new")).replace(main_config)
            print("✅ Kubeconfig merged successfully")
        else:
            print(f"⚠️  Warning: Failed to merge kubeconfig: {result.stderr}")
    else:
        print(f"📝 Creating new config: {main_config}")
        shutil.copy(kubeconfig_path, main_config)
        print("✅ Kubeconfig copied to main config")
    
    # Rename context
    print("🏷️  Renaming context...")
    result = run_command(
        f"kubectl config rename-context default k3s-{CLUSTER_NAME}",
        check=False
    )
    if result.returncode == 0:
        print(f"✅ Context renamed to: k3s-{CLUSTER_NAME}")
    else:
        # Context might already be renamed or not exist
        print(f"ℹ️  Context rename skipped (might already be renamed)")
    
    # Set as current context
    print("🎯 Setting as current context...")
    result = run_command(
        f"kubectl config use-context k3s-{CLUSTER_NAME}",
        check=False
    )
    if result.returncode == 0:
        print(f"✅ Current context set to: k3s-{CLUSTER_NAME}")
    else:
        print(f"⚠️  Warning: Failed to set context: {result.stderr}")
    
    print()
    print("✅ Done! Kubeconfig extracted and merged.")
    print(f"📋 Context name: k3s-{CLUSTER_NAME}")
    print(f"📁 Kubeconfig file: {kubeconfig_path}")
    print(f"📁 Main config: {main_config}")


if __name__ == "__main__":
    main()

