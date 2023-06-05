#!/bin/bash

# Prepares zombienet for usage
#
# This script is meant to be run from repository root directory

cd local-relay-network

# Download zombienet binary and make it executable
wget -nc https://github.com/paritytech/zombienet/releases/download/v1.3.39/zombienet-linux-x64
chmod +x zombienet-linux-x64


# Setup zombienet: prepare polkadot binary.
wget -nc https://github.com/paritytech/polkadot/releases/download/v0.9.40/polkadot
chmod +x polkadot

cd ..

docker build . -f docker/Dockerfile.aleph-collator -t aleph-parachain-node
