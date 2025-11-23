#!/usr/bin/env python3
"""
Setup Git credentials for FluxCD GitRepository from SOPS-encrypted .env file.

This script:
1. Reads git credentials from SOPS-encrypted .env file
2. Decrypts them using SOPS
3. Creates Kubernetes secrets for GitRepository authentication
4. Supports both HTTPS (username/password/token) and SSH (private key) authentication
"""

import os
import subprocess
import sys
from pathlib import Path


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_warn(msg):
    """Print warning message."""
    print(f"[WARN] {msg}")


def log_error(msg):
    """Print error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def run_command(cmd, check=True, capture_output=True, **kwargs):
    """Run a command and return the result."""
    result = subprocess.run(
        cmd,
        shell=isinstance(cmd, str),
        capture_output=capture_output,
        text=True,
        check=check,
        **kwargs
    )
    return result


def check_sops_installed():
    """Check if SOPS is installed."""
    result = run_command("sops --version", check=False, capture_output=True)
    if result.returncode != 0:
        log_error("SOPS is not installed")
        log_info("Install it with: brew install sops (macOS) or see https://github.com/mozilla/sops")
        return False
    log_info(f"SOPS found: {result.stdout.strip()}")
    return True


def decrypt_env_file(env_file: Path) -> dict:
    """Decrypt SOPS-encrypted .env file and return as dictionary."""
    if not env_file.exists():
        log_warn(f".env file not found: {env_file}")
        log_info("Git credentials will not be configured")
        return {}
    
    log_info(f"Decrypting .env file: {env_file}")
    
    # Decrypt using SOPS
    result = run_command(
        ["sops", "-d", str(env_file)],
        check=False,
        capture_output=True
    )
    
    if result.returncode != 0:
        log_error(f"Failed to decrypt .env file: {result.stderr}")
        log_info("Make sure SOPS is configured and GPG keys are available")
        return {}
    
    # Parse decrypted content
    env_vars = {}
    for line in result.stdout.splitlines():
        line = line.strip()
        # Skip comments and empty lines
        if not line or line.startswith('#'):
            continue
        # Parse KEY=VALUE format
        if '=' in line:
            key, value = line.split('=', 1)
            env_vars[key.strip()] = value.strip()
    
    log_info(f"Decrypted {len(env_vars)} environment variables")
    return env_vars


def create_https_secret(env_vars: dict, secret_name: str, namespace: str) -> bool:
    """Create Kubernetes secret for HTTPS git authentication."""
    # Check for HTTPS credentials
    # Support GITHUB_TOKEN as primary option (can be used with or without username)
    github_token = env_vars.get('GITHUB_TOKEN')
    git_token = env_vars.get('GIT_TOKEN')
    git_password = env_vars.get('GIT_PASSWORD')
    
    # Get username (optional for GitHub tokens)
    username = env_vars.get('GIT_USERNAME') or env_vars.get('GIT_USER')
    
    # Determine password/token
    password = github_token or git_token or git_password
    
    # For GitHub tokens, if no username provided, use token as username (GitHub accepts this)
    # Or use "git" as username (common pattern)
    if github_token and not username:
        username = github_token  # GitHub accepts token as username
        password = github_token
    
    if not password:
        log_info("No HTTPS git credentials found in .env")
        log_info("Supported variables: GITHUB_TOKEN, GIT_TOKEN, GIT_PASSWORD")
        log_info("Optional: GIT_USERNAME (defaults to token if using GITHUB_TOKEN)")
        return False
    
    if not username:
        # Fallback: use token as username (works for GitHub)
        username = password
    
    log_info(f"Creating HTTPS git credentials secret: {secret_name}")
    
    # Create secret using kubectl
    # Format: username=<username>\npassword=<password>
    credentials = f"username={username}\npassword={password}"
    
    result = run_command(
        [
            "kubectl", "create", "secret", "generic", secret_name,
            "--from-literal=username=" + username,
            "--from-literal=password=" + password,
            "-n", namespace,
            "--dry-run=client", "-o", "yaml"
        ],
        check=False,
        capture_output=True
    )
    
    if result.returncode != 0:
        log_error(f"Failed to generate secret YAML: {result.stderr}")
        return False
    
    # Apply the secret
    apply_result = run_command(
        ["kubectl", "apply", "-f", "-"],
        input=result.stdout,
        check=False,
        capture_output=True
    )
    
    if apply_result.returncode != 0:
        # Check if secret already exists
        if "already exists" in apply_result.stderr:
            log_warn(f"Secret {secret_name} already exists. Updating...")
            # Delete and recreate
            run_command(
                ["kubectl", "delete", "secret", secret_name, "-n", namespace],
                check=False
            )
            # Try again
            apply_result = run_command(
                ["kubectl", "apply", "-f", "-"],
                input=result.stdout,
                check=False,
                capture_output=True
            )
        
        if apply_result.returncode != 0:
            log_error(f"Failed to create secret: {apply_result.stderr}")
            return False
    
    log_info(f"✅ Created HTTPS git credentials secret: {secret_name}")
    return True


def create_ssh_secret(env_vars: dict, secret_name: str, namespace: str) -> bool:
    """Create Kubernetes secret for SSH git authentication."""
    # Check for SSH private key
    ssh_key = env_vars.get('GIT_SSH_KEY') or env_vars.get('GIT_SSH_PRIVATE_KEY')
    
    if not ssh_key:
        log_info("No SSH git credentials found in .env (GIT_SSH_KEY)")
        return False
    
    log_info(f"Creating SSH git credentials secret: {secret_name}")
    
    # Create secret using kubectl
    result = run_command(
        [
            "kubectl", "create", "secret", "generic", secret_name,
            "--from-literal=identity=" + ssh_key,
            "-n", namespace,
            "--dry-run=client", "-o", "yaml"
        ],
        check=False,
        capture_output=True
    )
    
    if result.returncode != 0:
        log_error(f"Failed to generate secret YAML: {result.stderr}")
        return False
    
    # Apply the secret
    apply_result = run_command(
        ["kubectl", "apply", "-f", "-"],
        input=result.stdout,
        check=False,
        capture_output=True
    )
    
    if apply_result.returncode != 0:
        # Check if secret already exists
        if "already exists" in apply_result.stderr:
            log_warn(f"Secret {secret_name} already exists. Updating...")
            # Delete and recreate
            run_command(
                ["kubectl", "delete", "secret", secret_name, "-n", namespace],
                check=False
            )
            # Try again
            apply_result = run_command(
                ["kubectl", "apply", "-f", "-"],
                input=result.stdout,
                check=False,
                capture_output=True
            )
        
        if apply_result.returncode != 0:
            log_error(f"Failed to create secret: {apply_result.stderr}")
            return False
    
    log_info(f"✅ Created SSH git credentials secret: {secret_name}")
    return True


def main():
    """Main function."""
    import argparse
    
    parser = argparse.ArgumentParser(
        description="Setup Git credentials for FluxCD GitRepository from SOPS-encrypted .env file"
    )
    parser.add_argument(
        "--env-file",
        type=Path,
        default=Path(".env"),
        help="Path to SOPS-encrypted .env file (default: .env)"
    )
    parser.add_argument(
        "--secret-name",
        default="git-credentials",
        help="Kubernetes secret name (default: git-credentials)"
    )
    parser.add_argument(
        "--namespace",
        default="flux-system",
        help="Kubernetes namespace (default: flux-system). Use comma-separated list for multiple namespaces."
    )
    parser.add_argument(
        "--also-namespace",
        action="append",
        help="Additional namespace to create secret in (can be used multiple times, e.g., --also-namespace tilt --also-namespace dev)"
    )
    parser.add_argument(
        "--all-environments",
        action="store_true",
        help="Create secrets in all environment namespaces (tilt, dev, stage, prod) plus flux-system and microscaler-system"
    )
    parser.add_argument(
        "--auth-type",
        choices=["auto", "https", "ssh"],
        default="auto",
        help="Authentication type: auto (detect), https, or ssh (default: auto)"
    )
    
    args = parser.parse_args()
    
    log_info("Git Credentials Setup Script")
    log_info("=" * 50)
    
    # Check prerequisites
    if not check_sops_installed():
        sys.exit(1)
    
    # Decrypt .env file
    env_vars = decrypt_env_file(args.env_file)
    
    if not env_vars:
        log_warn("No environment variables found. Git credentials will not be configured.")
        log_info("To configure git credentials, add to .env file:")
        log_info("  For GitHub (recommended): GITHUB_TOKEN=ghp_...")
        log_info("  For generic HTTPS: GIT_USERNAME=... and GIT_TOKEN=...")
        log_info("  For SSH: GIT_SSH_KEY=...")
        sys.exit(0)
    
    # Determine namespaces to create secrets in
    namespaces = [args.namespace]
    
    if args.all_environments:
        # Create secrets in all environment namespaces
        environment_namespaces = ["tilt", "dev", "stage", "prod", "microscaler-system"]
        namespaces.extend(environment_namespaces)
        # Remove duplicates while preserving order
        namespaces = list(dict.fromkeys(namespaces))
    elif args.also_namespace:
        # Add additional namespaces specified via --also-namespace
        namespaces.extend(args.also_namespace)
        # Remove duplicates while preserving order
        namespaces = list(dict.fromkeys(namespaces))
    
    # Create secrets based on auth type in all specified namespaces
    created = False
    
    for namespace in namespaces:
        log_info(f"Processing namespace: {namespace}")
        
        # Check if namespace exists, create it if it doesn't (for environment namespaces)
        check_ns_result = run_command(
            f"kubectl get namespace {namespace}",
            check=False,
            capture_output=True
        )
        
        if check_ns_result.returncode != 0:
            # Try to create the namespace if it's an environment namespace
            if namespace in ["tilt", "dev", "stage", "prod"]:
                log_info(f"Creating namespace: {namespace}")
                create_ns_result = run_command(
                    f"kubectl create namespace {namespace}",
                    check=False,
                    capture_output=True
                )
                if create_ns_result.returncode != 0:
                    log_warn(f"⚠️  Could not create namespace {namespace}: {create_ns_result.stderr}")
                    log_warn(f"   Skipping secret creation in {namespace}")
                    continue
            else:
                log_warn(f"⚠️  Namespace {namespace} does not exist, skipping secret creation")
                log_warn(f"   (flux-system and microscaler-system should be created by their respective installers)")
                continue
        
        if args.auth_type == "auto":
            # Try HTTPS first, then SSH
            if create_https_secret(env_vars, args.secret_name, namespace):
                created = True
            elif create_ssh_secret(env_vars, args.secret_name, namespace):
                created = True
        elif args.auth_type == "https":
            if create_https_secret(env_vars, args.secret_name, namespace):
                created = True
        elif args.auth_type == "ssh":
            if create_ssh_secret(env_vars, args.secret_name, namespace):
                created = True
    
    if not created:
        log_warn("No git credentials found in .env file")
        log_info("Add credentials to .env file:")
        log_info("  GitHub (recommended): GITHUB_TOKEN=ghp_...")
        log_info("  Generic HTTPS: GIT_USERNAME=... and GIT_TOKEN=...")
        log_info("  SSH: GIT_SSH_KEY=...")
        sys.exit(0)
    
    log_info("")
    log_info("✅ Git credentials setup complete!")
    log_info(f"📋 Secret name: {args.secret_name}")
    log_info(f"📋 Namespaces: {', '.join(namespaces)}")
    log_info("")
    log_info("Next steps:")
    log_info("  1. Update GitRepository to reference this secret:")
    log_info(f"     secretRef:")
    log_info(f"       name: {args.secret_name}")
    log_info("  2. Verify secrets exist in all namespaces:")
    for namespace in namespaces:
        log_info(f"     kubectl get secret {args.secret_name} -n {namespace}")


if __name__ == "__main__":
    main()

