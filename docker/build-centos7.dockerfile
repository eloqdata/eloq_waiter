# syntax=docker/dockerfile:1.4

######################################################################
# Use a Docker ARG to detect architecture when building via buildx
######################################################################
FROM centos:7
ARG TARGETARCH 

RUN set -ex; \
    if [ -z "${TARGETARCH}" ]; then \
        echo "ERROR: TARGETARCH is empty!"; \
        exit 1; \
    fi; 

######################################################################
# Shared initial steps to fix CentOS repos and update packages
######################################################################
RUN set -ex \
    && sed -i s/mirror.centos.org/vault.centos.org/g /etc/yum.repos.d/*.repo \
    && sed -i s/^#.*baseurl=http/baseurl=http/g /etc/yum.repos.d/*.repo \
    && sed -i s/^mirrorlist=http/#mirrorlist=http/g /etc/yum.repos.d/*.repo \
    && yum update -y \
    && yum install -y epel-release \
    && yum clean all

######################################################################
# Conditional installation based on architecture
#   - arm64   -> Build GCC 11.3.0 from source
#   - x86_64  -> Install Devtoolset 11
######################################################################
ARG GCC_VERSION=11.3.0

RUN set -ex; \
    if [ "${TARGETARCH}" = "arm64" ]; then \
      echo "**** Building GCC from source for ARM64 ****"; \
      \
      yum groupinstall -y "Development Tools" && \
      yum install -y wget gcc gcc-c++ gmp-devel mpfr-devel libmpc-devel \
                     make flex bison texinfo libtool; \
      \
      # Download GCC
      cd /tmp && \
      wget https://ftp.gnu.org/gnu/gcc/gcc-${GCC_VERSION}/gcc-${GCC_VERSION}.tar.gz && \
      tar -xzf gcc-${GCC_VERSION}.tar.gz && cd gcc-${GCC_VERSION} && \
      ./contrib/download_prerequisites; \
      \
      # Build and install
      mkdir build && cd build && \
      ../configure --prefix=/opt/gcc-${GCC_VERSION} --disable-multilib --enable-languages=c,c++ && \
      make -j4 && \
      make install && \
      \
      # Clean up
      rm -rf /tmp/gcc-${GCC_VERSION}*; \
      \
      # Update environment
      echo "export PATH=/opt/gcc-${GCC_VERSION}/bin:\$PATH" >> /etc/bashrc; \
      echo "export LD_LIBRARY_PATH=/opt/gcc-${GCC_VERSION}/lib64:\$LD_LIBRARY_PATH" >> /etc/bashrc; \
      \
      # For non-interactive shells, set them here
      export PATH=/opt/gcc-${GCC_VERSION}/bin:$PATH; \
      export LD_LIBRARY_PATH=/opt/gcc-${GCC_VERSION}/lib64:$LD_LIBRARY_PATH; \
    else \
      echo "**** Installing Devtoolset 11 for x86_64 ****"; \
      \
      yum install -y scl-utils gcc gcc-c++ centos-release-scl; \
      # re-apply the mirror fix since we re-installed repos
      sed -i s/mirror.centos.org/vault.centos.org/g /etc/yum.repos.d/*.repo; \
      sed -i s/^#.*baseurl=http/baseurl=http/g /etc/yum.repos.d/*.repo; \
      sed -i s/^mirrorlist=http/#mirrorlist=http/g /etc/yum.repos.d/*.repo; \
      \
      yum install -y devtoolset-11-toolchain devtoolset-11-libasan-devel; \
      scl_source enable devtoolset-11; \
      # Disable SCL repos by default
      sed -i 's|enabled=1|enabled=0|g' /etc/yum.repos.d/CentOS-SCLo-scl*.repo; \
      \
    #   # Make devtoolset-11 permanent for PATH/LD_LIBRARY_PATH
    #   echo "source scl_source enable devtoolset-11" >> /etc/bashrc; \
    #   echo "if [ -f /opt/rh/devtoolset-11/enable ]; then . /opt/rh/devtoolset-11/enable; fi" >> /etc/bashrc; \
    #   \
    #   # For non-interactive shells in Docker
    #   source scl_source enable devtoolset-11; \
    #   # Also set the environment explicitly so subsequent RUN lines see it
    #   export PATH=/opt/rh/devtoolset-11/root/usr/bin:$PATH; \
    #   export LD_LIBRARY_PATH=/opt/rh/devtoolset-11/root/usr/lib64:$LD_LIBRARY_PATH; \
    fi

######################################################################
# Common development tools for both ARM and x86
######################################################################
RUN set -ex \
    && yum install -y \
        jq sudo vim wget gdb gnutls-devel bison ccache rsync \
        cmake3 ninja-build make openssh-clients leveldb-devel openssl-devel snappy-devel openssl \
        lcov bzip2-devel lz4-devel libasan.x86_64 ncurses-devel libuv-devel.x86_64 dh-autoreconf.noarch java-11-openjdk-devel \
        redis tcl readline-devel awscli patchelf ca-certificates pkg-config curl unzip \
    && yum install epel-release -y \
    && yum install git -y \
    && yum clean all \
    && ln -s /usr/bin/cmake3 /usr/bin/cmake

######################################################################
# Install AWS CLI v2
######################################################################
RUN set -ex \
    && curl "https://awscli.amazonaws.com/awscli-exe-linux-$(uname -m).zip" -o "awscliv2.zip" \
    && unzip awscliv2.zip && rm awscliv2.zip \
    && ./aws/install && rm -r aws

######################################################################
# Install Rust
######################################################################
RUN set -ex \
    && curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

######################################################################
# Compile Protobuf from source (v21.12) - same for both ARM and x86
#   This version is chosen to match both brpc and grpc compatibility
######################################################################
RUN set -ex \
    && echo "**** Testing Building protobuf from source ****" && \ 
    if [ "${TARGETARCH}" = "arm64" ]; then \
        mkdir -p $HOME/Downloads/protobuf \
        && cd $HOME/Downloads/protobuf \
        && curl -fsSL https://github.com/protocolbuffers/protobuf/archive/refs/tags/v21.12.tar.gz | \
        tar -xzf - --strip-components=1 \
        && cmake \
            -DCMAKE_BUILD_TYPE=Release \
            -DBUILD_SHARED_LIBS=yes \
            -Dprotobuf_BUILD_TESTS=OFF \
            -Dprotobuf_ABSL_PROVIDER=package \
            -S . -B cmake-out \
        && cmake --build cmake-out -- -j4 \
        && cmake --build cmake-out --target install -- -j4 \
        && ldconfig \
        && cd ../ && rm -rf protobuf; \
    else \
        source /opt/rh/devtoolset-11/enable && \
        gcc --version | grep -E '^gcc.* \(GCC\) 11\.' && \
        g++ --version | grep -E '^g\+\+.* \(GCC\) 11\.' ; \
        # source scl_source enable devtoolset-11 && \
        mkdir -p $HOME/Downloads/protobuf && cd $HOME/Downloads/protobuf && \
        curl -fsSL https://github.com/protocolbuffers/protobuf/archive/refs/tags/v21.12.tar.gz | \
        tar -xzf - --strip-components=1 && \
        cmake \
        -DCMAKE_BUILD_TYPE=Release \
        -DBUILD_SHARED_LIBS=yes \
        -Dprotobuf_BUILD_TESTS=OFF \
        -Dprotobuf_ABSL_PROVIDER=package \
        -S . -B cmake-out && \
        cmake --build cmake-out -- -j 4 && \
        cmake --build cmake-out --target install -- -j 4 && \
        ldconfig && \
        cd ../ && rm -rf protobuf; \
    fi

# RUN set -ex \
#     && mkdir -p $HOME/Downloads/protobuf \
#     && cd $HOME/Downloads/protobuf \
#     && curl -fsSL https://github.com/protocolbuffers/protobuf/archive/refs/tags/v21.12.tar.gz | \
#        tar -xzf - --strip-components=1 \
#     && cmake \
#         -DCMAKE_BUILD_TYPE=Release \
#         -DBUILD_SHARED_LIBS=yes \
#         -Dprotobuf_BUILD_TESTS=OFF \
#         -Dprotobuf_ABSL_PROVIDER=package \
#         -S . -B cmake-out \
#     && cmake --build cmake-out -- -j"$(nproc)" \
#     && cmake --build cmake-out --target install -- -j"$(nproc)" \
#     && ldconfig \
#     && cd ../ && rm -rf protobuf

######################################################################
# Final environment variables
# Depending on the path changes, you may want to ensure devtoolset/gcc
# environment is always visible in Docker. The simplest approach is to
# rely on the environment modifications in /etc/bashrc plus re-exports:
######################################################################
# ENV PATH="/opt/gcc-${GCC_VERSION}/bin:/opt/rh/devtoolset-11/root/usr/bin:${PATH}"
# ENV LD_LIBRARY_PATH="/opt/gcc-${GCC_VERSION}/lib64:/opt/rh/devtoolset-11/root/usr/lib64:${LD_LIBRARY_PATH}"

######################################################################
# Entry point (optional). Often just a shell.
######################################################################
# CMD ["/bin/bash"]





# seg fault for building amd protoc on arm
# 330.8 [ 37%] Building CXX object CMakeFiles/libprotobuf.dir/src/google/protobuf/util/internal/default_value_objectwriter.cc.o
# 331.0 c++: internal compiler error: Segmentation fault (program cc1plus)
# 331.1 Please submit a full bug report,
# 331.1 with preprocessed source if appropriate.
# 331.1 See <http://bugzilla.redhat.com/bugzilla> for instructions.
# 331.1 gmake[2]: *** [CMakeFiles/libprotobuf.dir/src/google/protobuf/util/internal/default_value_objectwriter.cc.o] Error 4
# 331.1 gmake[2]: *** Waiting for unfinished jobs....
# 338.9 gmake[1]: *** [CMakeFiles/libprotobuf.dir/all] Error 2
# 338.9 gmake: *** [all] Error 2
# ------
