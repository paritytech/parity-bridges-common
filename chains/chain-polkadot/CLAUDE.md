# chain-polkadot

`no_std` primitives for the **Polkadot** relay chain, used on-chain in bridge pallets.

## Re-exports

`bp_polkadot_core::*` — shares core types with Kusama.

## Key constants

- Chain ID: `*b"pdot"`
- `PARAS_PALLET_NAME` = `"Paras"`
- `WITH_POLKADOT_GRANDPA_PALLET_NAME` = `"BridgePolkadotGrandpa"`
- `WITH_POLKADOT_BRIDGE_PARACHAINS_PALLET_NAME` = `"BridgePolkadotParachains"`
- `MAX_NESTED_PARACHAIN_HEAD_DATA_SIZE` = `128`

## Traits

`Chain`, `ChainWithGrandpa`

## Note

Uses `SuffixedCommonTransactionExtension<PrevalidateAttests>` (unique to Polkadot).

## When to update

On Polkadot runtime upgrades — pallet names or chain parameters may change.
