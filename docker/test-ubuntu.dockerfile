ARG UBT_ID=24.04
FROM ubuntu:$UBT_ID

RUN set -eux; \
    apt update; \
    export DEBIAN_FRONTEND=noninteractive; \
    apt install -y --no-install-recommends sudo wget curl ca-certificates openssh-server iproute2 redis-tools git rsync python3; \
    rm -rf /var/lib/apt/lists/*;

RUN useradd -rm -s /bin/bash -g sudo eloquser && \
    echo '%sudo ALL=(ALL) NOPASSWD:ALL' >> /etc/sudoers

USER eloquser
WORKDIR /home/eloquser

USER root
RUN mkdir -p /home/eloquser/.ssh && \
    ssh-keygen -t rsa -N '' -f /home/eloquser/.ssh/id_rsa && \
    cat /home/eloquser/.ssh/id_rsa.pub > /home/eloquser/.ssh/authorized_keys && \
    chown -R eloquser /home/eloquser/.ssh && chmod 400 /home/eloquser/.ssh/* && \
    mkdir /run/sshd

USER eloquser

EXPOSE 22
CMD ["/usr/sbin/sshd", "-D"]
