#!/usr/bin/env python3
"""
Pre-commit hook for Rust code formatting and checking.

Runs cargo fmt and cargo check on the secret-manager-controller.

Usage:
    This script is called by pre-commit framework automatically
    Can also be run manually: python3 scripts/pre_commit_rust.py
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_error(msg):
    """Print error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def log_warn(msg):
    """Print warning message."""
    print(f"[WARN] {msg}")


def main():
    """Main pre-commit function."""
    # Check if cargo is available
    if not shutil.which("cargo"):
        log_error("cargo is not installed. Please install Rust: https://rustup.rs/")
        sys.exit(1)
    
    script_dir = Path(__file__).parent
    controller_dir = script_dir.parent
    
    os.chdir(controller_dir)
    
    log_info("Running cargo fmt on secret-manager-controller...")
    result = subprocess.run(
        ["cargo", "fmt", "--check", "--all"],
        capture_output=True,
        text=True
    )
    
    if result.returncode != 0:
        log_error("Code formatting check failed. Run 'cargo fmt' to fix formatting issues.")
        log_info("Attempting to auto-format...")
        subprocess.run(["cargo", "fmt", "--all"], check=True)
        log_error("Code has been auto-formatted. Please review changes and commit again.")
        sys.exit(1)
    
    log_info("Running cargo check on secret-manager-controller...")
    result = subprocess.run(
        ["cargo", "check", "--all-targets"],
        capture_output=True,
        text=True
    )
    
    if result.returncode != 0:
        log_error("cargo check failed. Please fix compilation errors before committing.")
        if result.stdout:
            print(result.stdout)
        if result.stderr:
            print(result.stderr, file=sys.stderr)
        sys.exit(1)
    
    log_info("Rust code formatting and checks passed!")
    sys.exit(0)


if __name__ == "__main__":
    main()

