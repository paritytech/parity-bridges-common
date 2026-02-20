# substrate-relay

Main CLI binary orchestrating all bridge relay operations.

## Entry point

`main.rs` → `cli/mod.rs` (parses `Command` enum) → bridge-specific modules. Signal handling via `signal-hook` for graceful shutdown on SIGINT/SIGTERM.

## CLI subcommands

- `relay-headers-and-messages` — **primary production mode**, runs header + message relays bidirectionally
- `relay-headers` / `relay-header` — continuous / one-shot header relay
- `relay-parachains` / `relay-parachain-head` — continuous / one-shot parachain head relay
- `relay-messages` / `relay-messages-range` — continuous / ranged message relay
- `relay-messages-delivery-confirmation` — relay delivery confirmations
- `init-bridge` — bootstrap on-chain bridge pallet with current header
- `detect-equivocations` — scan for GRANDPA equivocations

## Bridge modules (`src/bridges/`)

| Module | Type | Chains |
|--------|------|--------|
| `kusama_polkadot` | parachain↔parachain | BridgeHubKusama ↔ BridgeHubPolkadot |
| `rococo_westend` | parachain↔parachain | BridgeHubRococo ↔ BridgeHubWestend |
| `polkadot_bulletin` | relay↔parachain | PolkadotBulletin ↔ BridgeHubPolkadot |
| `rococo_bulletin` | relay↔parachain | RococoBulletin ↔ BridgeHubRococo |

Each module contains relay definitions for headers, parachains, and messages in both directions. Relay files use macros like `generate_submit_finality_proof_ex_call_builder!` and `generate_receive_message_proof_call_builder!` to generate on-chain call builders.
