#!/bin/bash
# A script for relaying Millau messages to the Rialto chain.
#
# Will not work unless both the Rialto and Millau are running (see `run-rialto-node.sh`
# and `run-millau-node.sh).
set -xeu

MILLAU_PORT="${MILLAU_PORT:-9945}"
RIALTO_PORT="${RIALTO_PORT:-9944}"

RUST_LOG=bridge=debug \
./target/debug/substrate-relay relay-messages millau-to-rialto \
	--lane "b3849561e1a6169bee5a337422f3dbe93c9d385494c24483d380f35671774fb1" \
	--source-host localhost \
	--source-port $MILLAU_PORT \
	--source-signer //Bob \
	--target-host localhost \
	--target-port $RIALTO_PORT \
	--target-signer //Bob \
	--prometheus-host=0.0.0.0
