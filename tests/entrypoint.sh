#!/bin/bash
set -e

# Install the mounted public key
if [ -f /tmp/ssh_key.pub ]; then
    cp /tmp/ssh_key.pub /home/eloquser/.ssh/authorized_keys
    chown eloquser /home/eloquser/.ssh/authorized_keys
    chmod 600 /home/eloquser/.ssh/authorized_keys
else
    echo "WARNING: No SSH public key mounted at /tmp/ssh_key.pub"
fi

# Ensure sshd can start
mkdir -p /run/sshd

# Generate host keys if missing
ssh-keygen -A

echo "Starting SSH daemon..."
exec /usr/sbin/sshd -D
