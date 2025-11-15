#!/usr/bin/env python3
"""
Docker build with cleanup and timestamp.

This script replaces the inline shell script in custom_build for Docker builds.
It handles:
- Cleaning up old images
- Building Docker image with timestamp
- Tagging and pushing to registry
"""

import os
import subprocess
import sys
import time


def run_command(cmd_list, check=False, capture_output=True):
    """Run a command as a list (not shell string) and return the result."""
    result = subprocess.run(cmd_list, capture_output=capture_output, text=True)
    if not capture_output:
        return result
    if result.stdout:
        print(result.stdout, end="")
    if result.stderr and result.returncode != 0:
        print(result.stderr, end="", file=sys.stderr)
    return result


def main():
    """Main Docker build function."""
    image_name = os.getenv("IMAGE_NAME", "localhost:5002/secret-manager-controller")
    controller_name = os.getenv("CONTROLLER_NAME", "secret-manager-controller")
    controller_dir = os.getenv("CONTROLLER_DIR", ".")
    expected_ref = os.getenv("EXPECTED_REF", f"{image_name}:tilt")
    
    # Cleanup before build
    print("🧹 Cleaning up old images...")
    run_command(["docker", "rmi", f"{image_name}:tilt"], check=False)
    
    # Remove all tilt-* tags
    list_tags_result = run_command(
        ["docker", "images", image_name, "--format", "{{.Tag}}"],
        check=False
    )
    if list_tags_result.returncode == 0 and list_tags_result.stdout:
        for tag in list_tags_result.stdout.strip().split("\n"):
            tag = tag.strip()
            if tag.startswith("tilt-"):
                run_command(["docker", "rmi", f"{image_name}:{tag}"], check=False)
                run_command(["docker", "rmi", f"localhost:5002/{controller_name}:{tag}"], check=False)
    
    run_command(["docker", "rmi", f"localhost:5002/{controller_name}:tilt"], check=False)
    
    # Clean up kind registry cache
    run_command(
        ["docker", "exec", "kind-registry", "sh", "-c", f"rm -rf /var/lib/registry/docker/registry/v2/repositories/{controller_name}/"],
        check=False
    )
    
    # Build with timestamp
    timestamp = str(int(time.time()))
    dockerfile_path = os.path.join(controller_dir, "Dockerfile.dev")
    image_tag = f"{image_name}:tilt-{timestamp}"
    
    print(f"🔨 Building Docker image with timestamp {timestamp}...")
    
    # Build Docker image
    build_result = run_command(
        ["docker", "build", "--no-cache", "-f", dockerfile_path, "-t", image_tag, controller_dir],
        check=False,
        capture_output=False
    )
    if build_result.returncode != 0:
        print("❌ Error: Docker build failed", file=sys.stderr)
        sys.exit(build_result.returncode)
    
    # Tag image
    tag_result = run_command(
        ["docker", "tag", image_tag, expected_ref],
        check=False
    )
    if tag_result.returncode != 0:
        print("❌ Error: Docker tag failed", file=sys.stderr)
        sys.exit(tag_result.returncode)
    
    # Push image
    push_result = run_command(
        ["docker", "push", expected_ref],
        check=False,
        capture_output=False
    )
    if push_result.returncode != 0:
        print("❌ Error: Docker push failed", file=sys.stderr)
        sys.exit(push_result.returncode)
    
    print(f"✅ Docker image built and pushed: {expected_ref}")


if __name__ == "__main__":
    main()
