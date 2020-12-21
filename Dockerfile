# NOTE Customize the binary that is being built by providing `PROJECT` build-arg.
# E.g. docker build --build-arg PROJECT=ethereum-poa-relay ...

# This first stage prepares our dependencies to be built by `cargo-chef`.
FROM rust as planner
WORKDIR /parity-bridges-common
RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# This second stage is where the dependencies actually get built.
# The reason we split it from the first stage is so that the `COPY . .`
# step doesn't blow our cache.
FROM bridge-deps AS cacher
WORKDIR /parity-bridges-common
RUN cargo install cargo-chef

COPY --from=planner /parity-bridges-common/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# In this third stage we go ahead and build the actual binary we want.
# This should be fairly quick since the dependencies are being built and
# cached in the previous stage.
FROM bridge-deps as builder
WORKDIR /parity-bridges-common
RUN cargo install cargo-chef

COPY . /parity-bridges-common
COPY --from=cacher /parity-bridges-common/target target
COPY --from=cacher $CARGO_HOME $CARGO_HOME

ARG PROJECT=ethereum-poa-relay
RUN cargo build --release --verbose -p ${PROJECT}
RUN strip ./target/release/${PROJECT}

# In this final stage we copy over the final binary and do some checks
# to make sure that everything looks good.
FROM ubuntu:xenial as runtime

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
