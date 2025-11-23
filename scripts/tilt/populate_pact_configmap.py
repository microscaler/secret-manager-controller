#!/usr/bin/env python3
"""
Populate and apply the pact-contracts ConfigMap from local pact files.

This script reads Pact JSON files from target/pacts/ and creates/updates
the pact-contracts ConfigMap in the cluster. It handles the case where
pact files don't exist yet (returns success but doesn't create ConfigMap).
"""

import json
import subprocess
import sys
from pathlib import Path


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_warn(msg):
    """Print warning message."""
    print(f"[WARN] {msg}", file=sys.stderr)


def log_error(msg):
    """Print error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def run_command(cmd, check=True, capture_output=True):
    """Run a shell command and return the result."""
    result = subprocess.run(
        cmd,
        shell=isinstance(cmd, str),
        capture_output=capture_output,
        text=True
    )
    if check and result.returncode != 0:
        log_error(f"Command failed: {cmd}")
        if result.stderr:
            log_error(result.stderr)
        sys.exit(1)
    return result


def find_pact_files(pact_dir: Path):
    """Find all Pact JSON files in the directory."""
    if not pact_dir.exists():
        return []
    
    pact_files = []
    for file_path in pact_dir.glob("*.json"):
        if file_path.is_file():
            pact_files.append(file_path)
    
    return sorted(pact_files)


def create_configmap_from_files(namespace: str, configmap_name: str, pact_files: list):
    """Create or update ConfigMap from pact files using kubectl create configmap --from-file."""
    if not pact_files:
        log_info("No pact files found - ConfigMap will remain empty")
        return True
    
    log_info(f"Found {len(pact_files)} pact file(s) to add to ConfigMap")
    
    # Get the directory containing the pact files
    pact_dir = pact_files[0].parent
    
    # Use kubectl create configmap --from-file pointing to the directory
    # This automatically adds all files in the directory as keys
    log_info(f"Creating/updating ConfigMap {namespace}/{configmap_name} from {pact_dir}...")
    
    # Use kubectl create with --dry-run and apply to handle both create and update
    # This is more reliable than delete/recreate
    log_info("Generating ConfigMap YAML...")
    dry_run_cmd = [
        "kubectl", "create", "configmap", configmap_name,
        "--namespace", namespace,
        "--from-file", str(pact_dir),
        "--dry-run=client",
        "-o", "yaml"
    ]
    
    result = run_command(dry_run_cmd, check=False, capture_output=True)
    if result.returncode != 0:
        log_error(f"Failed to generate ConfigMap YAML: {result.stderr}")
        return False
    
    # Apply the generated YAML (handles both create and update)
    log_info("Applying ConfigMap...")
    apply_cmd = [
        "kubectl", "apply", "-f", "-"
    ]
    apply_result = subprocess.run(
        apply_cmd,
        input=result.stdout,
        text=True,
        capture_output=True
    )
    
    if apply_result.returncode != 0:
        log_error(f"Failed to apply ConfigMap: {apply_result.stderr}")
        return False
    
    log_info(f"✅ ConfigMap {namespace}/{configmap_name} created/updated successfully")
    log_info(f"   Added {len(pact_files)} pact file(s)")
    for pact_file in pact_files:
        log_info(f"   - {pact_file.name}")
    
    return True


def main():
    """Main function."""
    namespace = "secret-manager-controller-pact-broker"
    configmap_name = "pact-contracts"
    pact_dir = Path("target/pacts")
    
    log_info("Populating pact-contracts ConfigMap from local pact files...")
    log_info(f"Pact directory: {pact_dir.absolute()}")
    
    # Find pact files
    pact_files = find_pact_files(pact_dir)
    
    if not pact_files:
        log_info("No pact files found in target/pacts/")
        log_info("This is expected if pact tests haven't run yet")
        log_info("ConfigMap will be created empty and populated when pact files are generated")
        
        # Check if ConfigMap already exists
        check_cmd = [
            "kubectl", "get", "configmap", configmap_name,
            "--namespace", namespace,
            "--ignore-not-found=true"
        ]
        result = run_command(check_cmd, check=False, capture_output=True)
        
        if result.returncode == 0 and configmap_name in result.stdout:
            log_info("ConfigMap already exists (empty) - no update needed")
        else:
            # Create empty ConfigMap if it doesn't exist
            log_info("Creating empty ConfigMap (will be populated when pact files are available)...")
            create_cmd = [
                "kubectl", "create", "configmap", configmap_name,
                "--namespace", namespace,
                "--from-literal", "placeholder=empty"
            ]
            result = run_command(create_cmd, check=False, capture_output=True)
            if result.returncode == 0:
                # Remove the placeholder
                patch_cmd = [
                    "kubectl", "patch", "configmap", configmap_name,
                    "--namespace", namespace,
                    "--type", "json",
                    "-p", '[{"op": "remove", "path": "/data/placeholder"}]'
                ]
                run_command(patch_cmd, check=False)
                log_info("✅ Empty ConfigMap created")
            elif "already exists" in result.stderr.lower():
                log_info("ConfigMap already exists")
            else:
                log_warn(f"Could not create ConfigMap: {result.stderr}")
        
        return 0
    
    # Create/update ConfigMap with pact files
    if create_configmap_from_files(namespace, configmap_name, pact_files):
        log_info("✅ ConfigMap populated and applied successfully")
        return 0
    else:
        log_error("Failed to populate ConfigMap")
        return 1


if __name__ == "__main__":
    sys.exit(main())

