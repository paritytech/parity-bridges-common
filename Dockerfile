FROM ubuntu:xenial AS builder
# NOTE Customize the binary that is being built by providing `PROJECT` build-arg.
# E.g. docker build --build-arg PROJECT=ethereum-poa-relay ...

# show backtraces
ENV RUST_BACKTRACE 1

ENV LAST_DEPS_UPDATE 2020-06-22

# install tools and dependencies
RUN set -eux; \
	apt-get update && \
	apt-get install -y file curl jq ca-certificates && \
	apt-get install -y cmake pkg-config libssl-dev git clang libclang-dev

ENV LAST_CERTS_UPDATE 2020-06-22

RUN update-ca-certificates && \
	curl https://sh.rustup.rs -sSf | sh -s -- -y

ENV PATH="/root/.cargo/bin:${PATH}"
ENV LAST_RUST_UPDATE 2020-06-22

RUN rustup update stable && \
	rustup install nightly && \
	rustup target add wasm32-unknown-unknown --toolchain nightly

RUN rustc -vV && \
    cargo -V && \
    gcc -v && \
    g++ -v && \
    cmake --version

WORKDIR /parity-bridges-common

### Build locally
ADD . /parity-bridges-common

ARG PROJECT=ethereum-poa-relay

RUN cargo build --release --verbose -p ${PROJECT}
RUN strip ./target/release/${PROJECT}

FROM ubuntu:xenial

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
COPY ./deployments/scripts/bridge-entrypoint.sh ./

# check if executable works in this container
RUN ./${PROJECT} --version

ENV PROJECT=$PROJECT
ENTRYPOINT ["/home/user/bridge-entrypoint.sh"]
