#!/usr/bin/env python3
"""
Build script for secret-manager-controller binary.

Cross-compiles the Rust binary for x86_64-unknown-linux-musl target.
Supports macOS (using cargo-zigbuild) and Linux (using musl-gcc).
"""

import os
import platform
import subprocess
import sys
from pathlib import Path


def run_command(cmd, check=True, capture_output=True):
    """Run a command and return the result."""
    result = subprocess.run(
        cmd,
        shell=True,
        capture_output=capture_output,
        text=True
    )
    if capture_output:
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr and result.returncode != 0:
            print(result.stderr, end="", file=sys.stderr)
    if check and result.returncode != 0:
        sys.exit(result.returncode)
    return result


def main():
    """Build the controller binary."""
    print("🔨 Building secret-manager-controller...")
    
    os_name = platform.system()
    arch = platform.machine()
    
    target = "x86_64-unknown-linux-musl"
    binary_path = Path(f"target/{target}/debug/secret-manager-controller")
    
    if os_name == "Darwin":
        # macOS: Use cargo zigbuild (like microservices)
        print("  Using cargo-zigbuild for cross-compilation (macOS)")
        result = run_command(f"cargo zigbuild --target {target}", check=False)
        if result.returncode != 0:
            print("❌ Build failed", file=sys.stderr)
            sys.exit(1)
    elif os_name == "Linux" and arch == "x86_64":
        # Linux x86_64: Use musl-gcc linker
        print("  Using musl-gcc linker (Linux x86_64)")
        env = os.environ.copy()
        env["CC_x86_64_unknown_linux_musl"] = "musl-gcc"
        env["CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER"] = "musl-gcc"
        result = subprocess.run(
            ["cargo", "build", "--target", target],
            env=env,
            capture_output=True,
            text=True
        )
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr:
            print(result.stderr, end="", file=sys.stderr)
        if result.returncode != 0:
            print("❌ Build failed", file=sys.stderr)
            sys.exit(1)
    else:
        # Fallback: Try regular cargo build
        print(f"  Using standard cargo build (OS: {os_name}, Arch: {arch})")
        result = run_command(f"cargo build --target {target}", check=False)
        if result.returncode != 0:
            print("❌ Build failed", file=sys.stderr)
            sys.exit(1)
    
    # Verify binary exists
    if not binary_path.exists():
        print(f"❌ Build failed: Binary not found at {binary_path}", file=sys.stderr)
        sys.exit(1)
    
    print(f"✅ Build complete: {binary_path}")


if __name__ == "__main__":
    main()

