#!/usr/bin/env python3
"""
Generate CRD using crdgen binary.

This script replaces the inline shell script in Tiltfile for CRD generation.
It handles:
- Running crdgen binary
- Validating generated YAML
- Applying CRD to Kubernetes cluster
"""

import os
import subprocess
import sys
from pathlib import Path


def main():
    """Main CRD generation function."""
    controller_dir = os.getenv("CONTROLLER_DIR", ".")
    crdgen_native_path = Path(controller_dir) / "target/debug/crdgen"
    crd_output_path = Path("config/crd/secretmanagerconfig.yaml")
    stderr_log_path = Path("/tmp/crdgen-stderr.log")
    
    # Ensure output directory exists
    crd_output_path.parent.mkdir(parents=True, exist_ok=True)
    
    # Check if native crdgen binary exists
    if not crdgen_native_path.exists():
        print(f"❌ Error: crdgen binary not found at {crdgen_native_path}", file=sys.stderr)
        print("   Make sure 'secret-manager-controller-build' has completed", file=sys.stderr)
        sys.exit(1)
    
    # Use native crdgen binary (runs on host, not in container)
    # Redirect stdout to CRD file, stderr to log file separately
    print("📋 Generating CRD...")
    env = os.environ.copy()
    env["RUST_LOG"] = "off"
    
    with open(crd_output_path, "w") as stdout_file, open(stderr_log_path, "w") as stderr_file:
        result = subprocess.run(
            [str(crdgen_native_path)],
            stdout=stdout_file,
            stderr=stderr_file,
            env=env
        )
    
    exit_code = result.returncode
    if exit_code != 0:
        print(f"❌ Error: CRD generation command failed with exit code {exit_code}", file=sys.stderr)
        if stderr_log_path.exists() and stderr_log_path.stat().st_size > 0:
            print("Error output:", file=sys.stderr)
            with open(stderr_log_path) as f:
                print(f.read(), file=sys.stderr)
        # Don't leave invalid YAML in the CRD file
        if crd_output_path.exists():
            crd_output_path.unlink()
        sys.exit(exit_code)
    
    # Validate CRD is valid YAML (must contain apiVersion, kind, or --- after comments)
    # Skip comment lines and check for actual YAML content
    if crd_output_path.exists():
        with open(crd_output_path) as f:
            lines = f.readlines()
            yaml_content_found = False
            for line in lines:
                stripped = line.strip()
                if stripped and not stripped.startswith("#"):
                    if stripped.startswith(("apiVersion", "kind", "---")):
                        yaml_content_found = True
                        break
        
        if not yaml_content_found:
            print("❌ Error: CRD generation failed - file does not contain valid YAML", file=sys.stderr)
            print("First 10 lines of output:", file=sys.stderr)
            with open(crd_output_path) as f:
                for i, line in enumerate(f):
                    if i >= 10:
                        break
                    print(line, end="", file=sys.stderr)
            sys.exit(1)
    
    print("✅ CRD generated successfully")
    
    # Delete existing CRD before applying (handles schema changes)
    print("📋 Deleting existing CRD (if exists)...")
    delete_result = subprocess.run(
        ["kubectl", "delete", "crd", "secretmanagerconfigs.secret-management.microscaler.io"],
        capture_output=True,
        text=True
    )
    # Ignore errors if CRD doesn't exist
    
    # Apply CRD to Kubernetes cluster
    print("📋 Applying CRD to cluster...")
    apply_result = subprocess.run(
        ["kubectl", "apply", "-f", str(crd_output_path)],
        capture_output=True,
        text=True
    )
    
    apply_exit_code = apply_result.returncode
    if apply_exit_code == 0:
        print("✅ CRD applied successfully")
    else:
        print(f"❌ Error: CRD apply failed with exit code {apply_exit_code}", file=sys.stderr)
        if apply_result.stderr:
            print(apply_result.stderr, file=sys.stderr)
        sys.exit(apply_exit_code)


if __name__ == "__main__":
    main()

