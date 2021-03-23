#!/bin/sh

# Run this script from root of the repo

cargo run --manifest-path=bin/rialto/node/Cargo.toml --release --features=runtime-benchmarks -- benchmark \
	--chain=local \
	--steps=50 \
	--repeat=20 \
	--pallet=pallet_bridge_messages \
	--extrinsic=* \
	--execution=wasm \
	--wasm-execution=Compiled \
	--heap-pages=4096 \
	--output=./modules/messages/src/weights.rs \
	--template=./.maintain/rialto-weight-template.hbs
