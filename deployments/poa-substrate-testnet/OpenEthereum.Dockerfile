FROM ubuntu:xenial AS builder

# show backtraces
ENV RUST_BACKTRACE 1

ENV LAST_DEPS_UPDATE 2020-06-19

# install tools and dependencies
RUN set -eux; \
	apt-get update && \
	apt-get install -y file curl jq ca-certificates && \
	apt-get install -y cmake pkg-config libssl-dev git clang libclang-dev

ENV LAST_CERTS_UPDATE 2020-06-19

RUN update-ca-certificates && \
	curl https://sh.rustup.rs -sSf | sh -s -- -y

ENV PATH="/root/.cargo/bin:${PATH}"
ENV LAST_RUST_UPDATE="2020-06-19"
RUN rustup update stable && \
	rustup install nightly && \
	rustup target add wasm32-unknown-unknown --toolchain nightly


RUN rustc -vV && \
    cargo -V && \
    gcc -v && \
    g++ -v && \
    cmake --version

ENV REPO https://github.com/svyatonik/parity.git
ENV HASH 9838f59b6536e7482e145fa55acf07ac4e824ed0

WORKDIR /openethereum
RUN git clone $REPO /openethereum
RUN git checkout $HASH

ENV BRIDGE_REPO https://github.com/paritytech/parity-bridges-common
ENV BRIDGE_HASH master

WORKDIR /parity-bridges-common
RUN git clone $BRIDGE_REPO /parity-bridges-common
RUN git checkout $BRIDGE_HASH

WORKDIR /openethereum

RUN cargo build --release --verbose
RUN strip ./target/release/openethereum

FROM ubuntu:xenial

# show backtraces
ENV RUST_BACKTRACE 1

RUN groupadd -g 1000 openethereum \
  && useradd -u 1000 -g openethereum -s /bin/sh openethereum

# switch to user openethereum here
USER openethereum

WORKDIR /home/openethereum

COPY --chown=openethereum:openethereum --from=builder /openethereum/target/release/openethereum ./

# check if executable works in this container
RUN ./openethereum --version

EXPOSE 8545 8546 30303/tcp 30303/udp

ENTRYPOINT ["/home/openethereum/openethereum"]
