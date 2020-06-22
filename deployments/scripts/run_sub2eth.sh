#!/bin/bash
set -e

rm -rf substrate_alice.db
rm -rf substrate_bob.db
rm -rf substrate_charlie.db
rm -rf substrate_dave.db
rm -rf substrate_eve.db
rm -rf parity.db

test -f parity || (echo "Compile Parity with Bridge builtin support first"; exit 1)

cargo build --manifest-path=../../Cargo.toml -p bridge-node
cp ../../target/debug/bridge-node .
cargo build --manifest-path=../../Cargo.toml -p ethereum-poa-relay
cp ../../target/debug/ethereum-poa-relay .

# Start Substrate and Parity nodes
RUST_LOG=runtime=debug unbuffer ./bridge-node --chain=local --alice -d substrate_alice.db 2>&1 | unbuffer -p gawk '{ print strftime("Substrate.Alice: [%Y-%m-%d %H:%M:%S]"), $0 }' | unbuffer -p tee substrate_alice.log&
RUST_LOG=runtime=debug unbuffer ./bridge-node --chain=local --bob -d substrate_bob.db 2>&1 | unbuffer -p gawk '{ print strftime("Substrate.Bob: [%Y-%m-%d %H:%M:%S]"), $0 }' | unbuffer -p tee substrate_bob.log&
RUST_LOG=runtime=debug unbuffer ./bridge-node --chain=local --charlie -d substrate_charlie.db 2>&1 | unbuffer -p gawk '{ print strftime("Substrate.Charlie: [%Y-%m-%d %H:%M:%S]"), $0 }' | unbuffer -p tee substrate_charlie.log&
RUST_LOG=runtime=debug unbuffer ./bridge-node --chain=local --dave -d substrate_dave.db 2>&1 | unbuffer -p gawk '{ print strftime("Substrate.Dave: [%Y-%m-%d %H:%M:%S]"), $0 }' | unbuffer -p tee substrate_dave.log&
RUST_LOG=runtime=debug unbuffer ./bridge-node --chain=local --eve -d substrate_eve.db 2>&1 | unbuffer -p gawk '{ print strftime("Substrate.Eve: [%Y-%m-%d %H:%M:%S]"), $0 }' | unbuffer -p tee substrate_eve.log&
unbuffer ./parity -d parity.db --chain dev --force-sealing --no-warp --no-persistent-txqueue --jsonrpc-apis=all --jsonrpc-cors=all 2>&1 | unbuffer -p gawk '{ print strftime("Parity: [%Y-%m-%d %H:%M:%S]"), $0 }' | unbuffer -p tee parity.log&


# Deploy bridge contract
sleep 10
RUST_LOG=bridge=trace ./ethereum-poa-relay eth-deploy-contract  2>&1 | unbuffer -p gawk '{ print strftime("Substrate: [%Y-%m-%d %H:%M:%S]"), $0 }' | unbuffer -p tee deploy.log&

# Start syncing Substrate -> Ethereum
sleep 10
RUST_LOG=bridge=trace unbuffer ./ethereum-poa-relay sub-to-eth 2>&1 | unbuffer -p gawk '{ print strftime("Bridge: [%Y-%m-%d %H:%M:%S]"), $0 }' | unbuffer -p tee bridge.log&
