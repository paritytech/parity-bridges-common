#!/bin/bash
set -xeu

RUST_LOG=bridge=debug \
./target/debug/substrate-relay relay-messages millau-to-rialto \
	--lane 00000000 \
	--millau-host localhost \
	--millau-port 9945 \
	--millau-signer //Eve \
	--rialto-host localhost \
	--rialto-port 9944 \
	--rialto-signer //Eve \
	--prometheus-host=0.0.0.0
