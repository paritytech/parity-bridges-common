#!/bin/bash
set -xeu

RUST_LOG=bridge=debug \
./target/debug/substrate-relay relay-messages rialto-to-millau \
	--lane 00000000 \
	--rialto-host localhost \
	--rialto-port 9944 \
	--rialto-signer //Ferdie \
	--millau-host localhost \
	--millau-port 9945 \
	--millau-signer //Ferdie \
	--prometheus-host=0.0.0.0
