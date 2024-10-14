#!/bin/bash
set -exo pipefail

# prepare environment
source /etc/os-release
if [ "$ID" == "centos" ] || [ "$ID" == "rocky" ] || [ "$ID" == "rhel" ]; then
    sudo /usr/sbin/sshd
elif [ "$ID" == "ubuntu" ]; then
    sudo service ssh start
fi

export ELOQCTL_HOME="${HOME}/.eloqctl"
export PATH="$PATH:$ELOQCTL_HOME/bin"

# Find the file named 'url' inside the 'eloqctl-tarball-*' directories
VERSION_FILE=$(find . -type f -path "./eloqctl-tarball-*/url" 2>/dev/null)

# Check if the version file was found
if [ -z "$VERSION_FILE" ]; then
    echo "Version file not found"
    exit 1
fi

# TODO(ZX) extract ${ARCH}, ${TAG} and ${OS_ID} to use in pipeline

# Extract version from the version file
version_id=$(sed -n 's|.*eloqctl-\([0-9]\+\.[0-9]\+\.[0-9]\+\)-.*|\1|p' "$VERSION_FILE")

# Check if version was extracted successfully
if [ -z "$version_id" ]; then
    echo "Failed to extract version from the version file"
    exit 1
fi

echo "Extracted version: $version_id"

sudo chown -R $(whoami) waiter_src
cd waiter_src
git checkout "$version_id"
bash ./concourse/install.sh "$version_id"

cd "$ELOQCTL_HOME"
cat version

# Run the 'launch.sh' script first to install dependencies
bash tests/launch.sh

# Loop through all .sh files in the 'tests' directory
for script in tests/*.sh; do
    # Skip 'launch.sh' and 'external_cass.sh'
    if [[ "$script" == "tests/launch.sh" || "$script" == "tests/external_cass.sh" ]]; then
        continue
    fi

    # Execute the script
    bash "$script"
done

# Check if Python version is not 3.12
if [[ ! "$(python3 --version)" =~ "Python 3.12" ]]; then
    wget https://downloads.datastax.com/enterprise/cqlsh-astra.tar.gz
    tar -xzvf cqlsh-astra.tar.gz
    export PATH=$PATH:${PWD}/cqlsh-astra/bin
    bash tests/external_cass.sh 172.31.5.203
fi
