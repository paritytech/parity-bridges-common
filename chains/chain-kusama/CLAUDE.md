# chain-kusama

`no_std` primitives for the **Kusama** relay chain, used on-chain in bridge pallets.

## Re-exports

`bp_polkadot_core::*` — shares core types with Polkadot.

## Key constants

- Chain ID: `*b"ksma"`
- `PARAS_PALLET_NAME` = `"Paras"`
- `WITH_KUSAMA_GRANDPA_PALLET_NAME` = `"BridgeKusamaGrandpa"`
- `WITH_KUSAMA_BRIDGE_PARACHAINS_PALLET_NAME` = `"BridgeKusamaParachains"`
- `MAX_NESTED_PARACHAIN_HEAD_DATA_SIZE` = `128`

## Traits

`Chain`, `ChainWithGrandpa`

## When to update

On Kusama runtime upgrades — pallet names or chain parameters may change.
