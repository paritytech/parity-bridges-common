#!/bin/bash
set -e

SKIP_WASM_BUILD=1

rm -rf substrate.db
rm -rf parity.db

test -f parity || (echo "Compile Parity with Bridge builtin support first"; exit 1)

cargo build --manifest-path=../../Cargo.toml -p bridge-node
cp ../../target/debug/bridge-node .
cargo build --manifest-path=../../Cargo.toml -p ethereum-poa-relay
cp ../../target/debug/ethereum-poa-relay .

unbuffer ./parity -d parity.db --chain kovan --no-warp --no-persistent-txqueue --jsonrpc-apis=all 2>&1 | unbuffer -p gawk '{ print strftime("Parity: [%Y-%m-%d %H:%M:%S]"), $0 }' | unbuffer -p tee parity.log&
RUST_LOG=runtime=trace,rpc=debug,txpool=trace,basic_authorship=trace unbuffer ./bridge-node --execution=Native --dev -d substrate.db 2>&1 | unbuffer -p gawk '{ print strftime("Substrate: [%Y-%m-%d %H:%M:%S]"), $0 }' | unbuffer -p tee substrate.log&
sleep 10
RUST_LOG=rpc=trace RUST_LOG=bridge=trace unbuffer ./ethereum-poa-relay eth-to-sub 2>&1 | unbuffer -p gawk '{ print strftime("Bridge: [%Y-%m-%d %H:%M:%S]"), $0 }' | unbuffer -p tee bridge.log&
