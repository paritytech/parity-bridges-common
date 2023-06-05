#!/bin/bash

# Runs local network with
# - 4 validator relay chain (rococo)
# - 2 collator aleph parachain
#
# Meant to be run from repository root directory

./scripts/zombienet_setup.sh

export PATH=$(pwd)/local-relay-network/:$PATH

zombienet-linux-x64 spawn --provider native local-relay-network/config.toml
