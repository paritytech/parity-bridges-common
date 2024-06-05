#!/bin/bash

cd tools/runtime-codegen
cargo run --bin runtime-codegen -- --from-node-url "wss://rococo-bridge-hub-rpc.polkadot.io:443" > ../../relay-clients/client-bridge-hub-rococo/src/codegen_runtime.rs
cargo run --bin runtime-codegen -- --from-node-url "wss://rococo-rpc.polkadot.io:443" > ../../relay-clients/client-rococo/src/codegen_runtime.rs

cargo run --bin runtime-codegen -- --from-node-url "wss://westend-rpc.polkadot.io:443" > ../../relay-clients/client-westend/src/codegen_runtime.rs
cargo run --bin runtime-codegen -- --from-node-url "wss://westend-bridge-hub-rpc.polkadot.io:443" > ../../relay-clients/client-bridge-hub-westend/src/codegen_runtime.rs

cargo run --bin runtime-codegen -- --from-node-url "wss://kusama-rpc.polkadot.io" > ../../relay-clients/client-kusama/src/codegen_runtime.rs
cargo run --bin runtime-codegen -- --from-node-url "wss://kusama-bridge-hub-rpc.polkadot.io" > ../../relay-clients/client-bridge-hub-kusama/src/codegen_runtime.rs

cargo run --bin runtime-codegen -- --from-node-url "wss://dot-rpc.stakeworld.io" > ../../relay-clients/client-polkadot/src/codegen_runtime.rs
cargo run --bin runtime-codegen -- --from-node-url "wss://polkadot-bridge-hub-rpc.polkadot.io" > ../../relay-clients/client-bridge-hub-polkadot/src/codegen_runtime.rs

# Uncomment to update other runtimes

# For `polkadot-sdk` testnet runtimes:
# TODO: there is a bug, probably needs to update subxt, generates: `::sp_runtime::generic::Header<::core::primitive::u32>` withtout second `Hash` parameter.
# cargo run --bin runtime-codegen -- --from-wasm-file ../../../polkadot-sdk/target/release/wbuild/bridge-hub-rococo-runtime/bridge_hub_rococo_runtime.compact.compressed.wasm > ../../relays/client-bridge-hub-rococo/src/codegen_runtime.rs
# cargo run --bin runtime-codegen -- --from-wasm-file ../../../polkadot-sdk/target/release/wbuild/bridge-hub-westend-runtime/bridge_hub_westend_runtime.compact.compressed.wasm > ../../relays/client-bridge-hub-westend/src/codegen_runtime.rs

cd -
cargo +nightly fmt --all

# **IMPORTANT**: due to [well-known issue](https://github.com/paritytech/parity-bridges-common/issues/2669)
find . -name codegen_runtime.rs -exec \
    sed -i 's/::sp_runtime::generic::Header<::core::primitive::u32>/::sp_runtime::generic::Header<::core::primitive::u32, ::sp_runtime::traits::BlakeTwo256>/g' {} +

cargo +nightly fmt --all


# Polkadot Bulletin Chain:
#
# git clone https://github.com/zdave-parity/polkadot-bulletin-chain.git
# cd polkadot-bulletin-chain
# cargo run
# cargo run --bin runtime-codegen -- --from-node-url "ws://127.0.0.1:9944" > ../../relays/client-polkadot-bulletin/src/codegen_runtime.rs

cargo check --workspace