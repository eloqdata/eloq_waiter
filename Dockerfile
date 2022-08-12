# RUN Build:
#       $ docker build -f Dockerfile -t ${image_repo_name:tag} .
FROM ubuntu:20.04

RUN set -ex; \
    apt-get update --fix-missing; \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    sudo vim curl python3.8 libcurl4-openssl-dev libncurses5-dev \
    openssh-server openssh-client rsync libssl-dev openssl pkg-config; \
    rm -rf /var/lib/apt/lists/*

RUN echo 'root:PASSWORD' | chpasswd
# create admin user mono, default password cz123
RUN useradd -rm -s /bin/bash -g root -G sudo -p "$(openssl passwd -1 cz123)" mono && \
    echo '%sudo ALL=(ALL) NOPASSWD:ALL' >> /etc/sudoers && \
    sudo ln -s /usr/bin/python3.8 /usr/bin/python && \
    mkdir /var/crash && chown -R mono /var/crash


RUN mkdir /var/run/sshd

USER mono
WORKDIR /home/mono

RUN yes 'y' | ssh-keygen -q -t ed25519 -N '' -f /home/mono/.ssh/ed25519 > /dev/null

RUN sudo sed -i 's/PermitRootLogin prohibit-password/PermitRootLogin yes/' /etc/ssh/sshd_config

RUN sudo sed 's@session\s*required\s*pam_loginuid.so@session optional pam_loginuid.so@g' -i /etc/pam.d/sshd

EXPOSE 80 433 22 3317 3318 3319
# install rust
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
RUN echo 'source $HOME/.cargo/env' >> $HOME/.bashrc

RUN	sudo ssh-keygen -A

CMD ["/usr/bin/sudo", "/usr/sbin/sshd", "-D", "-o", "ListenAddress=0.0.0.0"]