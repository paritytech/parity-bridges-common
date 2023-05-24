# Builds images used by the bridge using locally built binaries.
#
# In particular, it can be used to build Substrate nodes and bridge relayers. The binary that gets
# built can be specified with the `PROJECT` build-arg. For example, to build the `substrate-relay`
# you would do the following:
#
# `docker build ./target -f local.Dockerfile -t local/substrate-relay --build-arg=PROJECT=substrate-relay`
#
# See the `deployments/README.md` for all the available `PROJECT` values.

FROM docker.io/library/ubuntu:20.04 as runtime

USER root
WORKDIR /home/root

# show backtraces
ENV RUST_BACKTRACE 1
ENV DEBIAN_FRONTEND=noninteractive

RUN set -eux; \
	apt-get update && \
	apt-get install -y --no-install-recommends \
        curl ca-certificates libssl-dev && \
    update-ca-certificates && \
	groupadd -g 1000 user && \
	useradd -u 1000 -g user -s /bin/sh -m user && \
	# apt clean up
	apt-get autoremove -y && \
	apt-get clean && \
	rm -rf /var/lib/apt/lists/*

# switch to non-root user
USER user

WORKDIR /home/user

ARG PROFILE=release
ARG PROJECT=substrate-relay

# assume that the host machine is binary compatible with Ubuntu
COPY --chown=user:user ./target/${PROFILE}/${PROJECT} ./

# check if executable works in this container
RUN ./${PROJECT} --version

ENV PROJECT=$PROJECT
ENTRYPOINT ["/bin/sh"]