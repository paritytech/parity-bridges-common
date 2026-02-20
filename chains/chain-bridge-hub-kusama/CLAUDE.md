# chain-bridge-hub-kusama

`no_std` primitives for the **BridgeHub-Kusama** parachain, used on-chain in bridge pallets.

## Re-exports

`bp_bridge_hub_cumulus::*`, `bp_messages::*`

## Key constants

- Chain ID: `*b"bhks"`
- `BRIDGE_HUB_KUSAMA_PARACHAIN_ID` = `1002`
- `WITH_BRIDGE_HUB_KUSAMA_MESSAGES_PALLET_NAME` = `"BridgeKusamaMessages"`
- `WITH_BRIDGE_HUB_KUSAMA_RELAYERS_PALLET_NAME` = `"BridgeRelayers"`

## Traits

`Chain`, `Parachain`, `ChainWithMessages`

## Note

Extrinsic size/weight limits are runtime-driven (reads `BlockLength`/`BlockWeights`).

## When to update

On BridgeHub-Kusama runtime upgrades — pallet names, parachain ID, or message parameters may change.
