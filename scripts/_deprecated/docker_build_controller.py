#!/usr/bin/env python3
"""
Docker build script for controller image (Tilt custom_build).

This script builds the Docker image for the controller, ensuring the binary exists
before building. It only rebuilds when the Dockerfile changes, not when the binary changes.
Binary updates are handled by Tilt's live_update sync.
"""

import os
import subprocess
import sys
from pathlib import Path


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
    """Build and tag Docker image for controller."""
    # Get image name from Tilt environment variable
    expected_ref = os.getenv('EXPECTED_REF')
    if not expected_ref:
        print("❌ ERROR: EXPECTED_REF environment variable not set", file=sys.stderr)
        sys.exit(1)
    
    # Parse image name and tag from EXPECTED_REF
    # Format: localhost:5000/secret-manager-controller:tilt-<hash>
    image_name = expected_ref.split(':')[0] + ':' + expected_ref.split(':')[1].split('@')[0]
    tag = expected_ref.split(':')[1].split('@')[0] if ':' in expected_ref else 'tilt'
    
    # Extract base image name (without tag)
    if ':' in image_name:
        base_image = image_name.rsplit(':', 1)[0]
    else:
        base_image = image_name
    
    # Check if binary exists
    binary_path = Path('build_artifacts/secret-manager-controller')
    if not binary_path.exists():
        print(f"❌ ERROR: Binary not found at {binary_path}", file=sys.stderr)
        print("   Make sure 'secret-manager-controller-build-and-copy' has run first", file=sys.stderr)
        sys.exit(1)
    
    print(f"🐳 Building Docker image: {base_image}:{tag}")
    print(f"   Binary: {binary_path} ({binary_path.stat().st_size / 1024 / 1024:.1f} MB)")
    
    # Build Docker image
    dockerfile = 'dockerfiles/Dockerfile.controller.dev'
    build_cmd = [
        'docker', 'build',
        '-f', dockerfile,
        '-t', f'{base_image}:{tag}',
        '.',  # Build context is project root
    ]
    
    result = run_command(build_cmd, check=False)
    if result.returncode != 0:
        print(f"❌ Docker build failed", file=sys.stderr)
        sys.exit(1)
    
    # Tag with EXPECTED_REF (required by Tilt)
    tag_cmd = ['docker', 'tag', f'{base_image}:{tag}', expected_ref]
    result = run_command(tag_cmd, check=False)
    if result.returncode != 0:
        print(f"❌ Failed to tag image: {expected_ref}", file=sys.stderr)
        sys.exit(1)
    
    # Push to registry (required by Tilt for custom_build)
    push_cmd = ['docker', 'push', expected_ref]
    result = run_command(push_cmd, check=False)
    if result.returncode != 0:
        print(f"❌ Failed to push image: {expected_ref}", file=sys.stderr)
        sys.exit(1)
    
    print(f"✅ Image built and pushed: {expected_ref}")


if __name__ == '__main__':
    main()

