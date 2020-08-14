#!/bin/bash

# Run a development instance of the Substrate bridge node.

RUST_LOG=runtime=trace,rpc=debug \
    ./target/debug/bridge-node --dev --tmp \
    --rpc-cors=all --unsafe-rpc-external --unsafe-ws-external
