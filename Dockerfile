# Builds images used by the bridge.
#
# In particular, it can be used to build Substrate nodes and bridge relayers. The binary that gets
# built can be specified with the `PROJECT` build-arg. For example, to build the `substrate-relay`
# you would do the following:
#
# `docker build . -t local/substrate-relay --build-arg=PROJECT=substrate-relay`
#
# See the `deployments/README.md` for all the available `PROJECT` values.

FROM docker.io/paritytech/bridge-dependencies:20.04 as builder
WORKDIR /parity-bridges-common
RUN cargo install cargo-chef --version 0.1.13

COPY . .

ARG PROJECT=ethereum-poa-relay
RUN cargo build --release --verbose -p ${PROJECT}
RUN strip ./target/release/${PROJECT}

# In this final stage we copy over the final binary and do some checks
# to make sure that everything looks good.
FROM docker.io/ubuntu:20.04 as runtime

# show backtraces
ENV RUST_BACKTRACE 1

RUN set -eux; \
	apt-get update && \
	apt-get install -y libssl-dev curl

RUN groupadd -g 1000 user \
  && useradd -u 1000 -g user -s /bin/sh -m user

# switch to non-root user
USER user

WORKDIR /home/user

ARG PROJECT=ethereum-poa-relay

COPY --chown=user:user --from=builder /parity-bridges-common/target/release/${PROJECT} ./
COPY --chown=user:user --from=builder /parity-bridges-common/deployments/local-scripts/bridge-entrypoint.sh ./

# check if executable works in this container
RUN ./${PROJECT} --version

ENV PROJECT=$PROJECT
ENTRYPOINT ["/home/user/bridge-entrypoint.sh"]
