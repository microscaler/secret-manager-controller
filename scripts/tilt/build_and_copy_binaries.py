#!/usr/bin/env python3
"""
Build and copy Rust binaries for secret-manager-controller.

This script combines building and copying binaries into a single step.
It handles:
- Cleaning old binaries
- Building Linux binaries (cross-compilation)
- Building native binaries (crdgen, msmctl)
- Copying binaries to build_artifacts
- Verifying binaries were created and copied
"""

import hashlib
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path


def run_command(cmd, check=True, shell=False, env=None, capture_output=True):
    """Run a command and return the result."""
    result = subprocess.run(cmd, shell=shell, check=check, capture_output=capture_output, text=True, env=env)
    if capture_output:
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr:
            print(result.stderr, end="", file=sys.stderr)
    return result


def get_md5_hash(filepath):
    """Calculate MD5 hash of a file."""
    hash_md5 = hashlib.md5()
    with open(filepath, "rb") as f:
        for chunk in iter(lambda: f.read(4096), b""):
            hash_md5.update(chunk)
    return hash_md5.hexdigest()


def get_file_size(filepath):
    """Get file size in bytes."""
    return Path(filepath).stat().st_size


def main():
    """Main build and copy function."""
    controller_dir = os.getenv("CONTROLLER_DIR", ".")
    binary_name = os.getenv("BINARY_NAME", "secret-manager-controller")
    
    # Paths
    linux_binary = Path(controller_dir) / "target/x86_64-unknown-linux-musl/debug" / binary_name
    linux_crdgen = Path(controller_dir) / "target/x86_64-unknown-linux-musl/debug/crdgen"
    linux_msmctl = Path(controller_dir) / "target/x86_64-unknown-linux-musl/debug/msmctl"
    native_crdgen = Path(controller_dir) / "target/debug/crdgen"
    native_msmctl = Path(controller_dir) / "target/debug/msmctl"
    
    artifact_path = Path("build_artifacts") / binary_name
    crdgen_artifact_path = Path("build_artifacts/crdgen")
    msmctl_artifact_path = Path("build_artifacts/msmctl")
    
    # ====================
    # Build Phase
    # ====================
    
    # Delete old binaries to force fresh build
    print("🧹 Cleaning old binaries from target directory...")
    for path in [linux_binary, linux_crdgen, linux_msmctl, native_crdgen, native_msmctl]:
        if path.exists():
            path.unlink()
    
    # Clean Cargo build artifacts
    print("🧹 Cleaning Cargo build artifacts...")
    clean_commands = [
        ["cargo", "clean", "-p", "secret-manager-controller", "--target", "x86_64-unknown-linux-musl"],
        ["cargo", "clean", "-p", "secret-manager-controller"],  # Clean native target as well
    ]
    for cmd in clean_commands:
        run_command(cmd, check=False)
    
    # Generate fresh timestamp for this build
    build_timestamp = str(int(time.time()))
    build_datetime = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    
    # Get git hash
    try:
        git_hash_result = run_command(["git", "rev-parse", "--short", "HEAD"], check=False)
        build_git_hash = git_hash_result.stdout.strip() if git_hash_result.returncode == 0 else "unknown"
    except Exception:
        build_git_hash = "unknown"
    
    # Check if git is dirty
    try:
        git_diff_result = run_command(["git", "diff", "--quiet"], check=False)
        build_git_dirty = "-dirty" if git_diff_result.returncode != 0 else ""
    except Exception:
        build_git_dirty = ""
    
    print("📋 Build info:")
    print(f"  Timestamp: {build_timestamp}")
    print(f"  DateTime: {build_datetime}")
    print(f"  Git Hash: {build_git_hash}{build_git_dirty}")
    
    # Build Linux binaries for container (cross-compilation)
    print("🔨 Building Linux binaries (debug mode)...")
    build_env = os.environ.copy()
    build_env["BUILD_TIMESTAMP"] = build_timestamp
    build_env["BUILD_DATETIME"] = build_datetime
    build_env["BUILD_GIT_HASH"] = f"{build_git_hash}{build_git_dirty}"
    
    # Use Python host-aware-build script
    build_script = Path(controller_dir) / "scripts/host_aware_build.py"
    if not build_script.exists():
        print(f"❌ Error: Build script not found at {build_script}", file=sys.stderr)
        sys.exit(1)
    
    build_result = run_command(
        ["python3", str(build_script), "--bin", binary_name, "--bin", "crdgen", "--bin", "msmctl", "--bin", "test-sops-decrypt"],
        check=False,
        env=build_env
    )
    if build_result.returncode != 0:
        print("❌ Error: Failed to build Linux binaries", file=sys.stderr)
        sys.exit(1)
    
    # Build native binaries for host execution (crdgen and msmctl)
    print("🔨 Building native binaries (crdgen, msmctl) (debug mode)...")
    cargo_build_env = os.environ.copy()
    cargo_build_env["BUILD_TIMESTAMP"] = build_timestamp
    cargo_build_env["BUILD_DATETIME"] = build_datetime
    cargo_build_env["BUILD_GIT_HASH"] = f"{build_git_hash}{build_git_dirty}"
    
    cargo_build_result = run_command(
        ["cargo", "build", "--bin", "crdgen", "--bin", "msmctl"],
        check=False,
        env=cargo_build_env
    )
    if cargo_build_result.returncode != 0:
        print("❌ Error: Failed to build native binaries", file=sys.stderr)
        sys.exit(1)
    
    # Verify binaries were created
    print("🔍 Verifying binaries were built...")
    build_error = False
    
    if not linux_binary.exists():
        print(f"❌ Error: Binary not found at {linux_binary}", file=sys.stderr)
        build_error = True
    else:
        print(f"  ✅ {binary_name} built successfully")
    
    if not linux_crdgen.exists():
        print(f"❌ Error: crdgen not found at {linux_crdgen}", file=sys.stderr)
        build_error = True
    else:
        print("  ✅ crdgen (Linux) built successfully")
    
    if not linux_msmctl.exists():
        print(f"❌ Error: msmctl (Linux) not found at {linux_msmctl}", file=sys.stderr)
        build_error = True
    else:
        print("  ✅ msmctl (Linux) built successfully")
    
    if not native_crdgen.exists():
        print(f"❌ Error: Native crdgen not found at {native_crdgen}", file=sys.stderr)
        build_error = True
    else:
        print("  ✅ crdgen (native) built successfully")
    
    if not native_msmctl.exists():
        print(f"❌ Error: Native msmctl not found at {native_msmctl}", file=sys.stderr)
        build_error = True
    else:
        print("  ✅ msmctl (native) built successfully")
    
    if build_error:
        print("❌ Build failed - some binaries are missing", file=sys.stderr)
        sys.exit(1)
    
    # ====================
    # Copy Phase
    # ====================
    
    # Ensure build_artifacts directory exists
    Path("build_artifacts").mkdir(parents=True, exist_ok=True)
    
    # Delete old binaries to ensure fresh copy
    print("🧹 Cleaning old binaries from build_artifacts...")
    for path in [artifact_path, crdgen_artifact_path, msmctl_artifact_path]:
        if path.exists():
            path.unlink()
    
    # Copy new binaries with error checking
    print("📋 Copying new binaries to build_artifacts...")
    copy_error = False
    
    # Use Python copy_binary script
    copy_script = Path("scripts/copy_binary.py")
    if not copy_script.exists():
        print(f"❌ Error: Copy script not found at {copy_script}", file=sys.stderr)
        sys.exit(1)
    
    # Copy main binary
    copy_result = run_command(
        ["python3", str(copy_script), str(linux_binary), str(artifact_path), binary_name],
        check=False,
        capture_output=True
    )
    if copy_result.returncode != 0:
        print(f"❌ Error: Failed to copy {binary_name}", file=sys.stderr)
        copy_error = True
    
    # Copy crdgen
    crdgen_copy_result = run_command(
        ["python3", str(copy_script), str(linux_crdgen), str(crdgen_artifact_path), "crdgen"],
        check=False,
        capture_output=True
    )
    if crdgen_copy_result.returncode != 0:
        print("❌ Error: Failed to copy crdgen", file=sys.stderr)
        copy_error = True
    
    # Copy msmctl
    msmctl_copy_result = run_command(
        ["python3", str(copy_script), str(linux_msmctl), str(msmctl_artifact_path), "msmctl"],
        check=False,
        capture_output=True
    )
    if msmctl_copy_result.returncode != 0:
        print("❌ Error: Failed to copy msmctl", file=sys.stderr)
        copy_error = True
    
    # Output hashes to verify what was copied
    print("")
    print("📊 Binary Hashes (verify what was built and copied):")
    binary_ok = False
    crdgen_ok = False
    msmctl_ok = False
    
    if artifact_path.exists():
        md5_hash = get_md5_hash(artifact_path)
        file_size = get_file_size(artifact_path)
        print(f"  {binary_name}: {md5_hash}")
        print(f"    Size: {file_size} bytes")
        binary_ok = True
    else:
        print(f"  ❌ {binary_name} not found!", file=sys.stderr)
        copy_error = True
    
    if crdgen_artifact_path.exists():
        md5_hash = get_md5_hash(crdgen_artifact_path)
        file_size = get_file_size(crdgen_artifact_path)
        print(f"  crdgen: {md5_hash}")
        print(f"    Size: {file_size} bytes")
        crdgen_ok = True
    else:
        print("  ❌ crdgen not found!", file=sys.stderr)
        copy_error = True
    
    if msmctl_artifact_path.exists():
        md5_hash = get_md5_hash(msmctl_artifact_path)
        file_size = get_file_size(msmctl_artifact_path)
        print(f"  msmctl: {md5_hash}")
        print(f"    Size: {file_size} bytes")
        msmctl_ok = True
    else:
        print("  ❌ msmctl not found!", file=sys.stderr)
        copy_error = True
    
    # Only report success if all binaries exist
    if copy_error or not binary_ok or not crdgen_ok or not msmctl_ok:
        print("❌ Binary copy failed - check errors above", file=sys.stderr)
        sys.exit(1)
    
    print("✅ Build and copy complete - all binaries verified")


if __name__ == "__main__":
    main()

