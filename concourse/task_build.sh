#!/bin/bash
set -exo pipefail

# Ensure cargo is installed (supports CentOS7/Rocky and Ubuntu 20/22/24)
if ! command -v cargo >/dev/null 2>&1; then
	echo "cargo not found, installing via rustup..."
	# Determine OS family and install prerequisites
	if [ -f /etc/os-release ]; then
		source /etc/os-release
	fi
	if [[ "${ID}" == "ubuntu" || "${ID_LIKE}" =~ debian ]]; then
		apt-get update
		DEBIAN_FRONTEND=noninteractive apt-get install -y curl build-essential pkg-config libssl-dev
	elif [[ "${ID}" == "centos" ]]; then
		yum install -y curl gcc gcc-c++ make pkgconfig openssl-devel
	elif [[ "${ID}" == "rocky" || "${ID_LIKE}" =~ rhel ]]; then
		dnf install -y curl gcc gcc-c++ make pkgconfig openssl-devel
	else
		echo "Unknown distro (${ID:-unknown}). Attempting to install curl..."
		(command -v apt-get >/dev/null 2>&1 && apt-get update && apt-get install -y curl) \
			|| (command -v yum >/dev/null 2>&1 && yum install -y curl) \
			|| (command -v dnf >/dev/null 2>&1 && dnf install -y curl) || true
	fi
	# Install rustup (non-interactive) and setup environment
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
	# shellcheck disable=SC1090
	[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
fi

# Make sure rustup env is loaded if present
# shellcheck disable=SC1090
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

cargo install cargo-make
cd monograph_waiter

# Determine OS version
source /etc/os-release
if [[ "$ID" == "centos" ]] || [[ "$ID" == "rocky" ]]; then
    OS_ID="rhel${VERSION_ID%.*}"
else
    OS_ID="${ID}${VERSION_ID%.*}"
fi

# Determine architecture
case $(uname -m) in
amd64 | x86_64) ARCH=amd64 ;;
arm64 | aarch64) ARCH=arm64 ;;
*) ARCH=$(uname -m) ;;
esac

# Handle tagged versions
if [ -n "${TAGGED}" ]; then
    TAG=$(git tag | sort -V | tail -n 1)
    if [ -z "${TAG}" ]; then
        echo "No tag found for HEAD. Exiting."
        exit 1
    fi
else
    TAG="main"
fi
TX_TARBALL="eloqctl-${TAG}-${OS_ID}-${ARCH}.tar.gz"

# Build
if [[ "$TAG" != "main" ]]; then
    echo "Checking out to $TAG..."
    git checkout "${TAG}"
else
    echo "TAG is 'main', no checkout performed."
fi
cargo make --no-workspace --makefile Makefile.toml rest_api_pkg
tar -czvf ../output/"${TX_TARBALL}" eloqctl

# Upload to S3
aws s3 cp ../output/"${TX_TARBALL}" s3://eloq-release/eloqctl/${ARCH}/${TAG}/${TX_TARBALL}
