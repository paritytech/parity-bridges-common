#!/bin/sh

# Run this script from root of the repo

# TODO: Interpreted must be changed to compiled after updating Substrate reference

cargo run --manifest-path=bin/rialto/node/Cargo.toml --release --features=runtime-benchmarks -- benchmark \
	--chain=local \
	--steps=50 \
	--repeat=20 \
	--pallet=pallet_message_lane \
	--extrinsic=* \
	--execution=wasm \
	--wasm-execution=Interpreted \
	--heap-pages=4096 \
	--output=./modules/message-lane/src/weights.rs \
	--template=./.maintain/rialto-weight-template.hbs
