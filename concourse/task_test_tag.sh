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

VERSION_FILE="eloqctl-tarball/url"
# Check if the version file exists
if [ ! -f "$VERSION_FILE" ]; then
    echo "Version file not found: $VERSION_FILE"
    exit 1
fi
version_id=$(sed -n 's|.*eloqctl-\([0-9]\+\.[0-9]\+\.[0-9]\+\).*|\1|p' "$VERSION_FILE")

bash waiter_src/concourse/install.sh "$version_id"

cd $ELOQCTL_HOME
cat version

# Q? this has to be in order?
bash tests/launch.sh
bash tests/demo.sh
bash tests/update.sh
bash tests/control.sh

# # Loop through all .sh files in the 'tests' directory
# for script in tests/*.sh; do
#     # Skip 'external_cass.sh'
#     if [[ "$script" == "tests/external_cass.sh" ]]; then
#         continue
#     fi

#     # Execute the script
#     bash "$script"
# done

# Check if Python version is not 3.12
if [[ ! "$(python3 --version)" =~ "Python 3.12" ]]; then
    wget https://downloads.datastax.com/enterprise/cqlsh-astra.tar.gz
    tar -xzvf cqlsh-astra.tar.gz
    export PATH=$PATH:${PWD}/cqlsh-astra/bin
    bash tests/external_cass.sh 172.31.5.203
fi
