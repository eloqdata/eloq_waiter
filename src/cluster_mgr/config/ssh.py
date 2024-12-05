#!/usr/bin/env python3

import os
import sys
import tempfile
import subprocess
import socket


def run_command(host, command):
    cmd = f'ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null {host} "{command}"'
    result = subprocess.run(
        cmd, shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    if result.returncode != 0:
        print(f"[-] Error running command on {host}: {result.stderr.decode()}")
        return None
    return result.stdout.decode()


def scp_file(host, src, dst):
    cmd = f"scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null {src} {host}:{dst}"
    result = subprocess.run(
        cmd, shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    if result.returncode != 0:
        print(f"[-] Error copying file to {host}: {result.stderr.decode()}")
        return False
    return True


def generate_local_ssh_key():
    ssh_dir = os.path.expanduser("~/.ssh")
    id_rsa = os.path.join(ssh_dir, "id_rsa")
    if not os.path.exists(id_rsa):
        os.system('ssh-keygen -t rsa -N "" -f {}'.format(id_rsa))


def get_local_public_key():
    id_rsa_pub = os.path.expanduser("~/.ssh/id_rsa.pub")
    with open(id_rsa_pub, "r") as f:
        return f.read().strip()


def get_host_keys(hosts):
    host_keys = {}
    for host in hosts:
        print(f"Collecting SSH host key from {host}")
        cmd = f"ssh-keyscan -H {host}"
        result = subprocess.run(
            cmd, shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        if result.returncode == 0:
            host_keys[host] = result.stdout.decode()
        else:
            print(
                f"[-] Error collecting host key from {host}: {result.stderr.decode()}"
            )
    return host_keys


def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} host1 [host2 ... hostN]")
        sys.exit(1)

    # Extract hosts from command-line arguments, skipping the script name
    hosts = [arg.strip() for arg in sys.argv[1:] if arg.strip()]

    if not hosts:
        print("No valid hosts provided.")
        sys.exit(1)

    # Include the coordinator's hostname
    coordinator_host = socket.gethostname()
    all_hosts = hosts + [coordinator_host]
    generate_local_ssh_key()
    local_pubkey = get_local_public_key()
    # Collect public keys from all hosts, including coordinator
    host_pubkeys = {}
    host_pubkeys[coordinator_host] = local_pubkey
    for host in hosts:
        print(f"Processing host {host}")
        # Ensure .ssh directory exists
        run_command(host, "mkdir -p ~/.ssh && chmod 700 ~/.ssh")
        # Generate SSH key on remote host if not exist
        run_command(
            host,
            "if [ ! -f ~/.ssh/id_rsa ]; then ssh-keygen -t rsa -N '' -f ~/.ssh/id_rsa; fi",
        )
        # Collect public key from host
        pubkey = run_command(host, "cat ~/.ssh/id_rsa.pub")
        if pubkey:
            host_pubkeys[host] = pubkey.strip()
        else:
            print(f"[-] Failed to get public key from {host}")
    # Combine all public keys
    all_pubkeys = set(host_pubkeys.values())
    # For each host, we need to merge existing authorized_keys with new keys
    for host in all_hosts:
        print(f"Updating authorized_keys on {host}")
        # Retrieve existing authorized_keys from host
        existing_keys = ""
        if host == coordinator_host:
            auth_keys_path = os.path.expanduser("~/.ssh/authorized_keys")
            if os.path.exists(auth_keys_path):
                with open(auth_keys_path, "r") as f:
                    existing_keys = f.read().strip().split("\n")
            else:
                existing_keys = []
        else:
            output = run_command(host, "cat ~/.ssh/authorized_keys 2>/dev/null")
            if output is not None:
                existing_keys = output.strip().split("\n")
            else:
                existing_keys = []
        # Combine existing keys with new keys
        combined_keys = set(existing_keys) | all_pubkeys
        # Write combined keys to a temporary file
        with tempfile.NamedTemporaryFile(mode="w+", delete=False) as tmpfile:
            tmpfile.write("\n".join(combined_keys) + "\n")
            tmpfile_path = tmpfile.name
        # Copy the combined authorized_keys back to the host
        if host == coordinator_host:
            # Copy to local ~/.ssh/authorized_keys
            os.replace(tmpfile_path, os.path.expanduser("~/.ssh/authorized_keys"))
            os.chmod(os.path.expanduser("~/.ssh/authorized_keys"), 0o600)
        else:
            scp_file(host, tmpfile_path, "~/.ssh/authorized_keys")
            run_command(host, "chmod 600 ~/.ssh/authorized_keys")
            # Clean up temporary file
            os.remove(tmpfile_path)
    # Collect SSH host keys from all hosts
    host_keys = get_host_keys(all_hosts)
    # For each host, update the known_hosts file
    for host in all_hosts:
        print(f"Updating known_hosts on {host}")
        # Retrieve existing known_hosts from host
        if host == coordinator_host:
            known_hosts_path = os.path.expanduser("~/.ssh/known_hosts")
            if os.path.exists(known_hosts_path):
                with open(known_hosts_path, "r") as f:
                    existing_known_hosts = f.read().strip().split("\n")
            else:
                existing_known_hosts = []
        else:
            output = run_command(host, "cat ~/.ssh/known_hosts 2>/dev/null")
            if output is not None:
                existing_known_hosts = output.strip().split("\n")
            else:
                existing_known_hosts = []
        # Combine existing known_hosts with collected host keys
        combined_known_hosts = set(existing_known_hosts)
        for h, key in host_keys.items():
            combined_known_hosts.update(key.strip().split("\n"))
        # Write combined known_hosts to a temporary file
        with tempfile.NamedTemporaryFile(mode="w+", delete=False) as tmpfile:
            tmpfile.write("\n".join(combined_known_hosts) + "\n")
            tmpfile_path = tmpfile.name
        # Copy the combined known_hosts back to the host
        if host == coordinator_host:
            # Copy to local ~/.ssh/known_hosts
            os.replace(tmpfile_path, os.path.expanduser("~/.ssh/known_hosts"))
            os.chmod(os.path.expanduser("~/.ssh/known_hosts"), 0o644)
        else:
            scp_file(host, tmpfile_path, "~/.ssh/known_hosts")
            run_command(host, "chmod 644 ~/.ssh/known_hosts")
            # Clean up temporary file
            os.remove(tmpfile_path)
    print("SSH key exchange and known_hosts update completed.")


if __name__ == "__main__":
    main()
