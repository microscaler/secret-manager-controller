#!/usr/bin/env python3
"""
Comprehensive Docker cleanup to prevent overwhelming Docker.

This script performs a full Docker purge routine:
1. Removes stopped containers (particularly Tilt build containers)
2. Prunes dangling images
3. Prunes unused images (older than 1 hour)
4. Prunes build cache
5. Prunes unused volumes
6. Prunes unused networks

It's safe to run repeatedly as it only removes unused resources.

Runs as a one-shot cleanup after controller builds complete.
"""

import subprocess
import sys
import os


def run_command(cmd, check=False, capture_output=True):
    """Run a command and return the result."""
    result = subprocess.run(cmd, capture_output=capture_output, text=True)
    if not capture_output:
        return result
    if result.stdout:
        print(result.stdout, end="")
    if result.stderr and result.returncode != 0:
        print(result.stderr, end="", file=sys.stderr)
    return result


def get_stopped_containers():
    """Get list of stopped container IDs."""
    result = run_command(
        ["docker", "ps", "-a", "--filter", "status=exited", "--format", "{{.ID}}"],
        check=False
    )
    if result.returncode != 0:
        return []
    
    container_ids = [line.strip() for line in result.stdout.strip().split("\n") if line.strip()]
    return container_ids


def get_container_info(container_id):
    """Get container name and image for a container ID."""
    result = run_command(
        ["docker", "inspect", "--format", "{{.Name}} {{.Config.Image}}", container_id],
        check=False
    )
    if result.returncode == 0 and result.stdout:
        return result.stdout.strip()
    return None


def cleanup_stopped_containers():
    """Remove stopped containers."""
    print("📦 Removing stopped containers...")
    
    stopped_containers = get_stopped_containers()
    
    if not stopped_containers:
        print("  ✅ No stopped containers found")
        return 0, 0
    
    print(f"  📋 Found {len(stopped_containers)} stopped container(s)")
    
    removed_count = 0
    failed_count = 0
    
    for container_id in stopped_containers:
        container_info = get_container_info(container_id)
        if container_info:
            container_name, image = container_info.split(" ", 1)
            # Log controller-related containers
            if "secret-manager-controller" in container_name or "secret-manager-controller" in image:
                print(f"    Removing: {container_name} ({image[:50]}...)")
        
        # Remove the container
        result = run_command(
            ["docker", "rm", container_id],
            check=False
        )
        
        if result.returncode == 0:
            removed_count += 1
        else:
            failed_count += 1
            if container_info:
                print(f"    ⚠️  Failed to remove: {container_info}", file=sys.stderr)
    
    print(f"  ✅ Removed {removed_count} container(s)")
    if failed_count > 0:
        print(f"  ⚠️  Failed to remove {failed_count} container(s)", file=sys.stderr)
    
    return removed_count, failed_count


def cleanup_dangling_images():
    """Remove dangling images (unused intermediate layers)."""
    print("🖼️  Pruning dangling images...")
    result = run_command(["docker", "image", "prune", "-f"], check=False)
    if result.stdout:
        # Extract reclaimed space from output
        output_lines = result.stdout.strip().split('\n')
        for line in output_lines:
            if 'reclaimed' in line.lower() or 'total' in line.lower():
                print(f"  {line.strip()}")
    return result.returncode == 0


def cleanup_unused_images():
    """Remove unused images (not used by any container, older than 1 hour).
    
    Note: This is a general Docker prune. Tilt-specific images are handled separately
    by cleanup_old_tilt_images() which checks running containers.
    """
    print("🖼️  Pruning unused images (older than 1 hour)...")
    result = run_command(
        ["docker", "image", "prune", "-a", "-f", "--filter", "until=1h"],
        check=False
    )
    if result.stdout:
        # Extract reclaimed space from output
        output_lines = result.stdout.strip().split('\n')
        for line in output_lines:
            if 'reclaimed' in line.lower() or 'total' in line.lower():
                print(f"  {line.strip()}")
    return result.returncode == 0


def cleanup_build_cache():
    """Prune build cache (keeps only last 1 hour for faster builds)."""
    print("🔨 Pruning build cache (keeping last 1 hour)...")
    result = run_command(
        ["docker", "builder", "prune", "-a", "-f", "--filter", "until=1h"],
        check=False
    )
    if result.stdout:
        # Extract reclaimed space from output
        output_lines = result.stdout.strip().split('\n')
        for line in output_lines:
            if 'reclaimed' in line.lower() or 'total' in line.lower():
                print(f"  {line.strip()}")
    return result.returncode == 0


def cleanup_unused_volumes():
    """Remove unused volumes."""
    print("💾 Pruning unused volumes...")
    result = run_command(["docker", "volume", "prune", "-f"], check=False)
    if result.stdout:
        # Extract reclaimed space from output
        output_lines = result.stdout.strip().split('\n')
        for line in output_lines:
            if 'reclaimed' in line.lower() or 'total' in line.lower():
                print(f"  {line.strip()}")
    return result.returncode == 0


def cleanup_unused_networks():
    """Remove unused networks."""
    print("🌐 Pruning unused networks...")
    result = run_command(["docker", "network", "prune", "-f"], check=False)
    if result.stdout:
        # Extract reclaimed space from output
        output_lines = result.stdout.strip().split('\n')
        for line in output_lines:
            if 'reclaimed' in line.lower() or 'total' in line.lower():
                print(f"  {line.strip()}")
    return result.returncode == 0


def get_running_container_images():
    """Get set of image references (repo:tag) currently used by running containers."""
    result = run_command(
        ["docker", "ps", "--format", "{{.Image}}"],
        check=False
    )
    if result.returncode != 0:
        return set()
    
    image_refs = [line.strip() for line in result.stdout.strip().split('\n') if line.strip()]
    return set(image_refs)


def cleanup_old_tilt_images():
    """Remove Tilt images that are not currently used by running containers."""
    print("🏷️  Removing unused Tilt images (not in use by running containers)...")
    image_name = os.getenv("IMAGE_NAME", "localhost:5000/secret-manager-controller")
    
    # Get all images for this image name
    result = run_command(
        ["docker", "images", image_name, "--format", "{{.ID}}\t{{.Repository}}\t{{.Tag}}"],
        check=False
    )
    
    if result.returncode != 0 or not result.stdout:
        print("  ✅ No Tilt images found")
        return True
    
    # Get set of image references currently in use by running containers
    running_image_refs = get_running_container_images()
    
    lines = [line.strip() for line in result.stdout.strip().split('\n') if line.strip()]
    if not lines:
        print("  ✅ No Tilt images found")
        return True
    
    removed_count = 0
    kept_count = 0
    
    for line in lines:
        parts = line.split('\t')
        if len(parts) < 3:
            continue
        
        image_id = parts[0]
        repository = parts[1]
        tag = parts[2]
        repo_tag = f"{repository}:{tag}"
        
        # Check if this image reference is currently in use by any running container
        # Also check if the image ID matches (in case tag changed but same image)
        is_in_use = repo_tag in running_image_refs
        
        # Also check by image ID - get all tags for running containers and compare IDs
        if not is_in_use:
            for running_ref in running_image_refs:
                # Get image ID for running container image
                inspect_result = run_command(
                    ["docker", "inspect", "--format", "{{.Id}}", running_ref],
                    check=False
                )
                if inspect_result.returncode == 0 and inspect_result.stdout:
                    running_image_id = inspect_result.stdout.strip()
                    # Compare full image IDs (they include sha256: prefix)
                    if image_id == running_image_id or image_id.startswith(running_image_id) or running_image_id.startswith(image_id):
                        is_in_use = True
                        break
        
        if is_in_use:
            kept_count += 1
            print(f"    Keeping (in use): {repo_tag}")
        else:
            # Remove unused image
            remove_result = run_command(["docker", "rmi", "-f", image_id], check=False)
            if remove_result.returncode == 0:
                removed_count += 1
                print(f"    Removed (unused): {repo_tag}")
            else:
                print(f"    ⚠️  Failed to remove: {repo_tag}", file=sys.stderr)
    
    print(f"  ✅ Removed {removed_count} unused Tilt image(s), kept {kept_count} in-use image(s)")
    return True


def main():
    """Main cleanup function - full purge routine."""
    print("🧹 Starting comprehensive Docker cleanup...")
    print("")
    
    total_errors = 0
    
    # 1. Remove stopped containers
    removed, failed = cleanup_stopped_containers()
    if failed > 0:
        total_errors += failed
    print("")
    
    # 2. Prune dangling images
    if not cleanup_dangling_images():
        total_errors += 1
    print("")
    
    # 3. Prune unused images
    if not cleanup_unused_images():
        total_errors += 1
    print("")
    
    # 4. Prune build cache
    if not cleanup_build_cache():
        total_errors += 1
    print("")
    
    # 5. Remove old Tilt images
    if not cleanup_old_tilt_images():
        total_errors += 1
    print("")
    
    # 6. Prune unused volumes
    if not cleanup_unused_volumes():
        total_errors += 1
    print("")
    
    # 7. Prune unused networks
    if not cleanup_unused_networks():
        total_errors += 1
    print("")
    
    print("✅ Comprehensive cleanup complete!")
    if total_errors > 0:
        print(f"⚠️  Encountered {total_errors} error(s) during cleanup", file=sys.stderr)
        return 1
    
    return 0


if __name__ == "__main__":
    sys.exit(main())

