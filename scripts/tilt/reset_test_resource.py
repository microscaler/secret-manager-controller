#!/usr/bin/env python3
"""
Reset test SecretManagerConfig resource.

This script replaces the inline shell script in Tiltfile for test resource management.
It handles:
- Deleting existing test resource
- Applying test resource from YAML
"""

import os
import subprocess
import sys
import time
from pathlib import Path


def main():
    """Main test resource reset function."""
    test_resource_yaml = Path("examples/test-sops-config.yaml")
    
    if not test_resource_yaml.exists():
        print(f"❌ Error: Test resource YAML not found at {test_resource_yaml}", file=sys.stderr)
        sys.exit(1)
    
    print("🔄 Resetting test SecretManagerConfig resource...")
    
    # Delete existing resource (ignore errors if it doesn't exist)
    print("📋 Deleting existing resource (if exists)...")
    delete_result = subprocess.run(
        ["kubectl", "delete", "secretmanagerconfig", "test-sops-config", "--ignore-not-found=true"],
        capture_output=True,
        text=True
    )
    # Ignore errors - resource may not exist
    
    # Wait a moment for deletion to complete
    time.sleep(1)
    
    # Apply the resource
    print("📋 Applying test SecretManagerConfig resource...")
    apply_result = subprocess.run(
        ["kubectl", "apply", "-f", str(test_resource_yaml)],
        capture_output=True,
        text=True
    )
    
    apply_exit_code = apply_result.returncode
    if apply_exit_code == 0:
        print("✅ Test resource applied successfully")
        print("📋 Resource: test-sops-config")
        print("📋 Namespace: default")
    else:
        print(f"❌ Error: Failed to apply test resource (exit code: {apply_exit_code})", file=sys.stderr)
        if apply_result.stderr:
            print(apply_result.stderr, file=sys.stderr)
        sys.exit(apply_exit_code)


if __name__ == "__main__":
    main()

