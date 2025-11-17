#!/usr/bin/env python3
"""
Add worker nodes to an existing k3s cluster.

This script creates 3 new Docker containers running k3s agent nodes
and joins them to the existing k3s cluster.
"""

import subprocess
import sys
import time
from pathlib import Path


CONTAINER_NAME = "k3s-secret-manager-controller"
NETWORK_NAME = "k3s-net"
NUM_NODES = 3


def run_command(cmd, check=True, capture_output=True):
    """Run a shell command."""
    result = subprocess.run(
        cmd,
        shell=True,
        capture_output=capture_output,
        text=True
    )
    if check and result.returncode != 0:
        print(f"❌ Error running command: {cmd}")
        print(f"   {result.stderr}")
        sys.exit(1)
    return result


def log_info(msg):
    """Print info message."""
    print(f"ℹ️  {msg}")


def log_error(msg):
    """Print error message."""
    print(f"❌ {msg}")


def log_success(msg):
    """Print success message."""
    print(f"✅ {msg}")


def get_node_token():
    """Get the k3s node token from the server container."""
    log_info("Retrieving k3s node token...")
    result = run_command(
        f"docker exec {CONTAINER_NAME} cat /var/lib/rancher/k3s/server/node-token",
        check=True
    )
    token = result.stdout.strip()
    log_success(f"Node token retrieved (length: {len(token)})")
    return token


def get_server_ip():
    """Get the k3s server IP address."""
    log_info("Getting k3s server IP address...")
    result = run_command(
        f"docker inspect {CONTAINER_NAME} --format='{{{{range .NetworkSettings.Networks}}}}{{{{.IPAddress}}}}{{{{end}}}}'",
        check=True
    )
    ip = result.stdout.strip()
    log_success(f"Server IP: {ip}")
    return ip


def check_network():
    """Check if k3s network exists."""
    result = run_command(
        f"docker network ls --format '{{{{.Name}}}}' | grep -q ^{NETWORK_NAME}$",
        check=False
    )
    if result.returncode != 0:
        log_error(f"Docker network '{NETWORK_NAME}' not found")
        log_info(f"Creating network '{NETWORK_NAME}'...")
        run_command(f"docker network create {NETWORK_NAME}", check=True)
        log_success(f"Network '{NETWORK_NAME}' created")
    else:
        log_success(f"Network '{NETWORK_NAME}' exists")


def create_worker_node(node_num, server_ip, node_token):
    """Create a k3s worker node container."""
    node_name = f"k3s-worker-{node_num}"
    
    # Check if node already exists
    result = run_command(
        f"docker ps -a --format '{{{{.Names}}}}' | grep -q ^{node_name}$",
        check=False
    )
    if result.returncode == 0:
        log_info(f"Node '{node_name}' already exists, removing...")
        run_command(f"docker rm -f {node_name}", check=False)
    
    log_info(f"Creating worker node {node_num}: {node_name}")
    
    # Create worker node container
    cmd = (
        f"docker run -d --name {node_name} --privileged --restart=unless-stopped "
        f"--network {NETWORK_NAME} "
        f"rancher/k3s:latest agent "
        f"--server https://{server_ip}:6443 "
        f"--token {node_token}"
    )
    
    run_command(cmd, check=True)
    log_success(f"Worker node '{node_name}' created")
    
    return node_name


def wait_for_node_ready(node_name, max_wait=120):
    """Wait for a node to become ready."""
    log_info(f"Waiting for node '{node_name}' to be ready...")
    
    for i in range(max_wait):
        # Check if container is running
        result = run_command(
            f"docker ps --format '{{{{.Names}}}}' | grep -q ^{node_name}$",
            check=False
        )
        if result.returncode != 0:
            log_error(f"Container '{node_name}' is not running")
            return False
        
        # Check if node appears in cluster (via server container)
        result = run_command(
            f"docker exec {CONTAINER_NAME} kubectl get nodes --no-headers 2>/dev/null | grep -q {node_name}",
            check=False
        )
        if result.returncode == 0:
            log_success(f"Node '{node_name}' is ready!")
            return True
        
        if i % 10 == 0 and i > 0:
            log_info(f"Still waiting... ({i}/{max_wait}s)")
        
        time.sleep(1)
    
    log_error(f"Node '{node_name}' did not become ready within {max_wait} seconds")
    return False


def main():
    """Main function."""
    print("🚀 Adding worker nodes to k3s cluster...")
    print()
    
    # Check if server container exists
    result = run_command(
        f"docker ps --format '{{{{.Names}}}}' | grep -q ^{CONTAINER_NAME}$",
        check=False
    )
    if result.returncode != 0:
        log_error(f"K3s server container '{CONTAINER_NAME}' not found or not running")
        log_info("Please start the k3s cluster first")
        sys.exit(1)
    
    log_success(f"Found k3s server container: {CONTAINER_NAME}")
    
    # Get cluster info
    node_token = get_node_token()
    server_ip = get_server_ip()
    
    # Check/create network
    check_network()
    
    print()
    log_info(f"Creating {NUM_NODES} worker nodes...")
    print()
    
    created_nodes = []
    for i in range(1, NUM_NODES + 1):
        node_name = create_worker_node(i, server_ip, node_token)
        created_nodes.append(node_name)
        time.sleep(2)  # Small delay between node creation
    
    print()
    log_info("Waiting for nodes to join the cluster...")
    print()
    
    # Wait for all nodes to be ready
    all_ready = True
    for node_name in created_nodes:
        if not wait_for_node_ready(node_name):
            all_ready = False
    
    print()
    if all_ready:
        log_success(f"All {NUM_NODES} worker nodes added successfully!")
        print()
        log_info("Current cluster nodes:")
        run_command(
            f"docker exec {CONTAINER_NAME} kubectl get nodes",
            check=False
        )
    else:
        log_error("Some nodes failed to join the cluster")
        log_info("Check node logs with:")
        for node_name in created_nodes:
            print(f"  docker logs {node_name}")
        sys.exit(1)


if __name__ == "__main__":
    main()

