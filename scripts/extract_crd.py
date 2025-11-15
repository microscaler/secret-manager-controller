#!/usr/bin/env python3
"""
Extract CRD from Docker image and clean it.

This script replaces extract-crd.sh and provides better error handling
and cross-platform support.

Usage: extract_crd.py <image-name> <output-path>
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def clean_ansi_sequences(content):
    """Remove ANSI escape sequences and control characters."""
    # Remove ANSI escape sequences: ESC[ followed by numbers and letters ending with m
    content = re.sub(r'\x1b\[[0-9;]*m', '', content)
    # Remove control characters (0x00-0x1F) except newlines (0x0A) and carriage returns (0x0D)
    content = re.sub(r'[\x00-\x08\x0B-\x0C\x0E-\x1F]', '', content)
    return content


def validate_yaml(content):
    """Validate that content appears to be valid YAML."""
    lines = content.strip().split('\n')
    for line in lines:
        stripped = line.strip()
        if stripped and not stripped.startswith('#'):
            if stripped.startswith(('apiVersion', 'kind', '---')):
                return True
    return False


def main():
    """Main extraction function."""
    if len(sys.argv) < 3:
        print("usage: extract_crd.py <image-name> <output-path>", file=sys.stderr)
        sys.exit(1)
    
    image_name = sys.argv[1]
    output_path = Path(sys.argv[2])
    container_name = f"crd-extract-{os.getpid()}"
    
    # Ensure output directory exists
    output_path.parent.mkdir(parents=True, exist_ok=True)
    
    # Extract CRD from Docker image
    print(f"📦 Extracting CRD from image: {image_name}")
    
    # Create container
    result = subprocess.run(
        ["docker", "create", "--name", container_name, image_name],
        capture_output=True,
        text=True
    )
    
    if result.returncode != 0:
        print("❌ Error: Failed to create container from image", file=sys.stderr)
        print(result.stderr, file=sys.stderr)
        sys.exit(1)
    
    try:
        # Copy CRD from container
        temp_file = output_path.with_suffix(output_path.suffix + ".tmp")
        result = subprocess.run(
            ["docker", "cp", f"{container_name}:/config/crd/secretmanagerconfig.yaml", str(temp_file)],
            capture_output=True,
            text=True
        )
        
        if result.returncode != 0:
            print("❌ Error: Failed to extract CRD from Docker image", file=sys.stderr)
            print(result.stderr, file=sys.stderr)
            sys.exit(1)
        
        # Read and clean content
        content = temp_file.read_text()
        cleaned_content = clean_ansi_sequences(content)
        
        # Validate it's valid YAML
        if not validate_yaml(cleaned_content):
            print("❌ Error: Extracted file does not appear to be valid YAML", file=sys.stderr)
            first_line = cleaned_content.split('\n')[0] if cleaned_content else ""
            print(f"First line: {first_line}", file=sys.stderr)
            print("File appears to contain logs instead of YAML. Check Dockerfile CRD generation step.", file=sys.stderr)
            temp_file.unlink()
            sys.exit(1)
        
        # Write cleaned content
        output_path.write_text(cleaned_content)
        temp_file.unlink()
        
        print(f"✅ CRD extracted and cleaned: {output_path}")
    
    finally:
        # Clean up container
        subprocess.run(
            ["docker", "rm", container_name],
            capture_output=True,
            check=False
        )


if __name__ == "__main__":
    import os
    main()

