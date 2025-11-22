#!/usr/bin/env python3
"""
Clean up containerd storage on Kind nodes to free disk space.

This script runs cleanup commands inside Kind node containers to free up
space in /var/lib/containerd/io.containerd.snapshotter.v1.overlayfs/
"""

import subprocess
import sys
from pathlib import Path


def run_command(cmd, check=False, capture_output=True):
    """Run a command and return the result."""
    result = subprocess.run(cmd, shell=True, capture_output=capture_output, text=True)
    if result.stdout:
        print(result.stdout, end="")
    if result.stderr and result.returncode != 0:
        print(result.stderr, end="", file=sys.stderr)
    return result


def get_kind_nodes():
    """Get list of Kind node container names."""
    result = run_command("kind get nodes --name secret-manager-controller", check=False)
    if result.returncode != 0:
        return []
    nodes = [line.strip() for line in result.stdout.strip().split('\n') if line.strip()]
    return nodes


def cleanup_containerd_storage(node):
    """Clean up containerd storage on a Kind node.
    
    Aggressively cleans up containerd storage to free disk space:
    - Removes unused images (including those not referenced by any container)
    - Removes unused snapshots
    - Removes unused content (blobs)
    """
    print(f"🧹 Cleaning up containerd storage on {node}...")
    
    # Run containerd garbage collection inside the node
    # This removes unused snapshots and blobs
    result = run_command(
        f"docker exec {node} ctr images prune --all",
        check=False
    )
    
    if result.returncode == 0:
        print(f"  ✅ Cleaned up unused images on {node}")
    else:
        print(f"  ⚠️  Warning: Failed to clean up images on {node}", file=sys.stderr)
    
    # Also try to clean up snapshots
    result = run_command(
        f"docker exec {node} ctr snapshots prune",
        check=False
    )
    
    if result.returncode == 0:
        print(f"  ✅ Cleaned up unused snapshots on {node}")
    else:
        print(f"  ⚠️  Warning: Failed to clean up snapshots on {node}", file=sys.stderr)
    
    # Aggressively clean up unused content (blobs)
    # This removes blobs that are not referenced by any image or snapshot
    result = run_command(
        f"docker exec {node} ctr content prune",
        check=False
    )
    
    if result.returncode == 0:
        print(f"  ✅ Cleaned up unused content (blobs) on {node}")
    else:
        print(f"  ⚠️  Warning: Failed to clean up content on {node}", file=sys.stderr)
    
    # Check disk usage
    result = run_command(
        f"docker exec {node} df -h /var/lib/containerd",
        check=False
    )
    if result.returncode == 0 and result.stdout:
        print(f"  📊 Disk usage on {node}:")
        for line in result.stdout.strip().split('\n'):
            if '/var/lib/containerd' in line or 'Filesystem' in line:
                print(f"    {line}")


def main():
    """Main cleanup function."""
    print("🧹 Cleaning up containerd storage on Kind nodes...")
    print("")
    
    nodes = get_kind_nodes()
    if not nodes:
        print("❌ No Kind nodes found for cluster 'secret-manager-controller'", file=sys.stderr)
        print("   Make sure the cluster is running: kind get clusters", file=sys.stderr)
        sys.exit(1)
    
    print(f"📋 Found {len(nodes)} node(s)")
    print("")
    
    for node in nodes:
        cleanup_containerd_storage(node)
        print("")
    
    print("✅ Cleanup complete!")


if __name__ == "__main__":
    main()

