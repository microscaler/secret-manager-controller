#!/usr/bin/env python3
"""
Build and push Secret Manager Controller Docker image.

This script replaces build-and-push.sh and provides better error handling
and cross-platform support.

Usage:
    build_and_push.py [image-tag] [registry]

Examples:
    build_and_push.py
    build_and_push.py v1.0.0
    build_and_push.py v1.0.0 ghcr.io/microscaler
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path


# Configuration
IMAGE_NAME = "secret-manager-controller"
DEFAULT_TAG = "latest"
DEFAULT_REGISTRY = "ghcr.io/microscaler"


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_warn(msg):
    """Print warning message."""
    print(f"[WARN] {msg}")


def log_error(msg):
    """Print error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def log_step(msg):
    """Print step message."""
    print(f"[STEP] {msg}")


def check_prerequisites():
    """Check prerequisites."""
    log_step("Checking prerequisites...")
    
    if not shutil.which("docker"):
        log_error("Docker is not installed")
        sys.exit(1)
    
    result = subprocess.run(
        ["docker", "buildx", "version"],
        capture_output=True,
        text=True
    )
    if result.returncode != 0:
        log_error("Docker buildx is not available")
        log_info("Install buildx: https://docs.docker.com/buildx/working-with-buildx/")
        sys.exit(1)
    
    log_info("Prerequisites check passed")


def setup_buildx():
    """Setup buildx builder."""
    log_step("Setting up buildx builder...")
    
    result = subprocess.run(
        ["docker", "buildx", "ls"],
        capture_output=True,
        text=True
    )
    
    if "secret-manager-builder" not in result.stdout:
        log_info("Creating buildx builder...")
        result = subprocess.run(
            ["docker", "buildx", "create", "--name", "secret-manager-builder", "--use"],
            capture_output=True,
            text=True
        )
        if result.returncode != 0:
            log_warn("Builder may already exist, using existing...")
            subprocess.run(["docker", "buildx", "use", "secret-manager-builder"], check=True)
    else:
        log_info("Using existing buildx builder")
        subprocess.run(["docker", "buildx", "use", "secret-manager-builder"], check=True)
    
    # Bootstrap builder
    subprocess.run(["docker", "buildx", "inspect", "--bootstrap"], check=True)


def build_and_push(tag, registry):
    """Build and push image."""
    log_step(f"Building and pushing image: {registry}/{IMAGE_NAME}:{tag}")
    
    script_dir = Path(__file__).parent
    controller_dir = script_dir.parent
    dockerfile = controller_dir / "Dockerfile"
    
    if not dockerfile.exists():
        log_error(f"Dockerfile not found at {dockerfile}")
        sys.exit(1)
    
    full_image_name = f"{registry}/{IMAGE_NAME}:{tag}"
    
    log_info("Building with docker buildx...")
    log_info(f"  Image: {full_image_name}")
    log_info(f"  Dockerfile: {dockerfile} (production multi-stage build)")
    log_info("  Platform: linux/amd64")
    
    # Build and push using buildx
    result = subprocess.run(
        [
            "docker", "buildx", "build",
            "--platform", "linux/amd64",
            "--file", str(dockerfile),
            "--tag", full_image_name,
            "--push",
            "--progress=plain",
            str(controller_dir)
        ],
        check=True
    )
    
    log_info("✅ Image built and pushed successfully!")
    log_info(f"   {full_image_name}")


def main():
    """Main execution."""
    tag = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_TAG
    registry = sys.argv[2] if len(sys.argv) > 2 else DEFAULT_REGISTRY
    
    log_info("Building and pushing Secret Manager Controller")
    print()
    
    check_prerequisites()
    setup_buildx()
    build_and_push(tag, registry)
    
    print()
    log_info("✅ Build and push complete!")
    log_info(f"   Image: {registry}/{IMAGE_NAME}:{tag}")


if __name__ == "__main__":
    main()

