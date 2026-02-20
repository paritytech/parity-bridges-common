# chain-bridge-hub-polkadot

`no_std` primitives for the **BridgeHub-Polkadot** parachain, used on-chain in bridge pallets.

## Re-exports

`bp_bridge_hub_cumulus::*`, `bp_messages::*`

## Key constants

- Chain ID: `*b"bhpd"`
- `BRIDGE_HUB_POLKADOT_PARACHAIN_ID` = `1002`
- `WITH_BRIDGE_HUB_POLKADOT_MESSAGES_PALLET_NAME` = `"BridgePolkadotMessages"`
- `WITH_BRIDGE_HUB_POLKADOT_RELAYERS_PALLET_NAME` = `"BridgeRelayers"`

## Traits

`Chain`, `Parachain`, `ChainWithMessages`

## Note

Extrinsic size/weight limits are runtime-driven (reads `BlockLength`/`BlockWeights`).

## When to update

On BridgeHub-Polkadot runtime upgrades — pallet names, parachain ID, or message parameters may change.
