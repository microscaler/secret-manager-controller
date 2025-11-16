#!/usr/bin/env python3
"""
Fix k3s registry configuration.

This script applies the registry configuration to the k3s container
so that k3s nodes can pull images from the local registry.
"""

import subprocess
import sys
import tempfile
import time
import os

CONTAINER_NAME = "k3s-secret-manager-controller"
REGISTRY_NAME = "secret-manager-controller-registry"
REGISTRY_PORT = "5002"


def run_command(cmd, check=True, input_text=None):
    """Run a command and return the result."""
    result = subprocess.run(
        cmd,
        shell=True,
        check=check,
        capture_output=True,
        text=True,
        input=input_text
    )
    return result


def main():
    """Main function."""
    print("🔧 Fixing k3s registry configuration...")
    
    # Check if k3s container exists
    result = run_command(f"docker ps -a --format '{{{{.Names}}}}' | grep -q ^{CONTAINER_NAME}$", check=False)
    if result.returncode != 0:
        print(f"❌ Error: K3s container '{CONTAINER_NAME}' not found")
        sys.exit(1)
    
    # Create registries.yaml
    registries_yaml = f"""mirrors:
  "localhost:{REGISTRY_PORT}":
    endpoint:
      - "http://{REGISTRY_NAME}:5000"
configs:
  "localhost:{REGISTRY_PORT}":
    tls:
      insecure_skip_verify: true
"""
    
    print("📝 Writing registries.yaml...")
    # Write to temp file and copy into container
    with tempfile.NamedTemporaryFile(mode='w', delete=False, suffix='.yaml') as f:
        f.write(registries_yaml)
        temp_yaml = f.name
    
    try:
        result = run_command(f"docker cp {temp_yaml} {CONTAINER_NAME}:/etc/rancher/k3s/registries.yaml")
        if result.returncode != 0:
            # Ensure directory exists first
            run_command(f"docker exec {CONTAINER_NAME} mkdir -p /etc/rancher/k3s")
            result = run_command(f"docker cp {temp_yaml} {CONTAINER_NAME}:/etc/rancher/k3s/registries.yaml")
        if result.returncode != 0:
            print(f"❌ Error: Failed to write registries.yaml: {result.stderr}")
            sys.exit(1)
    finally:
        os.unlink(temp_yaml)
    
    # Create hosts.toml
    hosts_toml = f"""server = "http://{REGISTRY_NAME}:5000"

[host."http://{REGISTRY_NAME}:5000"]
  capabilities = ["pull", "resolve"]
"""
    
    print("📝 Writing hosts.toml...")
    with tempfile.NamedTemporaryFile(mode='w', delete=False, suffix='.toml') as f:
        f.write(hosts_toml)
        temp_toml = f.name
    
    try:
        # Ensure directory exists first
        run_command(f"docker exec {CONTAINER_NAME} mkdir -p /var/lib/rancher/k3s/agent/etc/containerd/certs.d/localhost:{REGISTRY_PORT}")
        result = run_command(f"docker cp {temp_toml} {CONTAINER_NAME}:/var/lib/rancher/k3s/agent/etc/containerd/certs.d/localhost:{REGISTRY_PORT}/hosts.toml")
        if result.returncode != 0:
            print(f"❌ Error: Failed to write hosts.toml: {result.stderr}")
            sys.exit(1)
    finally:
        os.unlink(temp_toml)
    
    # Restart k3s to apply configuration
    print("🔄 Restarting k3s container...")
    result = run_command(f"docker restart {CONTAINER_NAME}", check=False)
    if result.returncode != 0:
        print(f"⚠️  Warning: Failed to restart k3s container: {result.stderr}")
    
    print("⏳ Waiting for k3s to be ready...")
    time.sleep(10)
    
    # Verify configuration
    print("✅ Verifying configuration...")
    result = run_command(f"docker exec {CONTAINER_NAME} cat /etc/rancher/k3s/registries.yaml", check=False)
    if result.returncode == 0 and REGISTRY_PORT in result.stdout:
        print("✅ Registry configuration applied successfully!")
        print(f"📋 Registries.yaml content:\n{result.stdout}")
    else:
        print("⚠️  Warning: Could not verify registries.yaml")
    
    print("✅ Done! K3s should now be able to pull images from localhost:5002")


if __name__ == "__main__":
    main()

