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
	--lane "17b6b4a8072ca3b1aee7b6fae09ac69a77c2b81bc6385b3c02798df2f64546f6" \
	--source-host localhost \
	--source-port $MILLAU_PORT \
	--source-signer //Bob \
	--target-host localhost \
	--target-port $RIALTO_PORT \
	--target-signer //Bob \
	--prometheus-host=0.0.0.0
