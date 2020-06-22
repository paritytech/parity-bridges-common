FROM ubuntu:xenial AS builder

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

ENV BRIDGE_REPO https://github.com/paritytech/parity-bridges-common
ENV BRIDGE_HASH master

WORKDIR /parity-bridges-common
RUN git clone $BRIDGE_REPO /parity-bridges-common
RUN git checkout $BRIDGE_HASH

RUN cargo build --release --verbose -p ethereum-poa-relay
RUN strip ./target/release/ethereum-poa-relay

FROM ubuntu:xenial

# show backtraces
ENV RUST_BACKTRACE 1

RUN set -eux; \
	apt-get update && \
	apt-get install -y libssl-dev

RUN groupadd -g 1000 user \
  && useradd -u 1000 -g user -s /bin/sh user

# switch to non-root user
USER user

WORKDIR /home/user

COPY --chown=user:user --from=builder /parity-bridges-common/target/release/ethereum-poa-relay ./

# check if executable works in this container
RUN ./ethereum-poa-relay --version

ENTRYPOINT ["/home/user/ethereum-poa-relay"]
