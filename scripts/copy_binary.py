#!/usr/bin/env python3
"""
Copy binary from build target to artifacts directory and create MD5 hash file.

This script replaces copy-binary.sh and provides better error handling and
cross-platform support.

Usage: copy_binary.py <target_path> <artifact_path> <binary_name>
"""

import hashlib
import os
import shutil
import sys
from pathlib import Path


def calculate_md5(filepath):
    """Calculate MD5 hash of a file."""
    hash_md5 = hashlib.md5()
    with open(filepath, "rb") as f:
        for chunk in iter(lambda: f.read(4096), b""):
            hash_md5.update(chunk)
    return hash_md5.hexdigest()


def main():
    """Main copy function."""
    if len(sys.argv) < 4:
        print("usage: copy_binary.py <target_path> <artifact_path> <binary_name>", file=sys.stderr)
        sys.exit(1)
    
    target_path = Path(sys.argv[1])
    artifact_path = Path(sys.argv[2])
    binary_name = sys.argv[3]
    hash_path = artifact_path.with_suffix(artifact_path.suffix + ".md5")
    
    # Create artifacts directory
    artifact_path.parent.mkdir(parents=True, exist_ok=True)
    
    # Check if source binary exists
    if not target_path.exists():
        print(f"❌ Error: {target_path} not found", file=sys.stderr)
        sys.exit(1)
    
    if not target_path.is_file():
        print(f"❌ Error: {target_path} is not a file", file=sys.stderr)
        sys.exit(1)
    
    # Delete existing binary from artifacts directory before copying
    if artifact_path.exists():
        artifact_path.unlink()
    
    # Copy binary to artifacts directory
    shutil.copy2(target_path, artifact_path)
    
    # Create MD5 hash file (triggers Docker rebuilds when binary changes)
    md5_hash = calculate_md5(artifact_path)
    hash_path.write_text(md5_hash + "\n")
    
    print(f"✅ Copied {binary_name}")
    print(f"   Source: {target_path}")
    print(f"   Artifact: {artifact_path}")
    print(f"   Hash: {hash_path}")


if __name__ == "__main__":
    main()

