# CLAUDE.md - Parity Bridges Common

## Project Overview

Parity Bridges Common is the bridge relay infrastructure for the Polkadot ecosystem. It provides the `substrate-relay` binary that relays finality proofs, parachain heads, messages, and equivocation reports between Substrate-based chains.

The relay binary connects pairs of chains, reading state from source chains and submitting transactions to target chains. It bundles hardcoded runtime metadata for each supported chain to construct transactions correctly.

## Supported Bridges

| Bridge | Chains | Environment | Lane |
|--------|--------|-------------|------|
| Kusama <> Polkadot | KusamaBridgeHub <> PolkadotBridgeHub | Production (`parity-chains`) | `00000001` |
| Rococo <> Westend | RococoBridgeHub <> WestendBridgeHub | Testnet (`parity-testnet`) | `00000002` |
| Polkadot Bulletin | PolkadotBridgeHub <> PolkadotBulletinChain | Production | - |
| Rococo Bulletin | RococoBridgeHub <> RococoBulletinChain | Testnet | - |

## Build Commands

```bash
# Check workspace compiles
cargo check --workspace

# Build release binary
cargo build --release

# Run all tests
cargo test --workspace

# Clippy linting
SKIP_WASM_BUILD=1 cargo clippy --all-targets --locked --workspace

# Format check
cargo +nightly fmt --all -- --check

# Check documentation builds
cargo doc --no-deps --all --workspace --document-private-items

# Dependency audit
cargo deny check advisories
cargo deny check bans sources
cargo deny check licenses
```

## Directory Structure

```
parity-bridges-common/
├── substrate-relay/              # Main relay binary
│   ├── Cargo.toml                # Version: 1.8.15
│   └── src/
│       ├── main.rs               # Entry point
│       ├── cli/                   # CLI command definitions
│       │   ├── mod.rs             # Command enum (all subcommands)
│       │   ├── init_bridge.rs     # Bootstrap bridge with finalized block
│       │   ├── relay_headers.rs   # Header relay (single + continuous)
│       │   ├── relay_messages.rs  # Message relay (continuous, range, confirmation)
│       │   ├── relay_parachains.rs        # Parachain head relay
│       │   ├── relay_headers_and_messages.rs  # High-level combined relay
│       │   ├── detect_equivocations.rs    # Equivocation detection + reporting
│       │   └── chain_schema.rs    # Chain connection parameters
│       └── bridges/               # Bridge-specific configurations
│           ├── kusama_polkadot/   # KBH <> PBH bridge modules
│           ├── polkadot_bulletin/ # PBH <> PBC bridge modules
│           ├── rococo_bulletin/   # RBH <> RBC bridge modules
│           └── rococo_westend/    # RBH <> WBH bridge modules
├── relay-clients/                # Chain client libraries (runtime metadata + types)
│   ├── client-bridge-hub-kusama/     # KBH: spec 2_000_006, tx 5
│   ├── client-bridge-hub-polkadot/   # PBH: spec 2_000_006, tx 4
│   ├── client-bridge-hub-rococo/     # RBH: spec 1_016_001, tx 6
│   ├── client-bridge-hub-westend/    # WBH: spec 1_016_001, tx 6
│   ├── client-kusama/                # Kusama: spec 1_002_004, tx 25
│   ├── client-polkadot/              # Polkadot: spec 1_003_003, tx 26
│   ├── client-polkadot-bulletin/     # PBC: spec 100, tx 1
│   ├── client-rococo/                # Rococo: spec 1_016_001, tx 26
│   ├── client-westend/               # Westend: spec 1_016_001, tx 26
│   ├── client-asset-hub-rococo/      # AHR: spec 1_017_001, tx 16
│   └── client-asset-hub-westend/     # AHW: spec 1_017_007, tx 16
├── chains/                       # Chain primitive types (headers, hashes)
│   ├── chain-bridge-hub-kusama/
│   ├── chain-bridge-hub-polkadot/
│   ├── chain-kusama/
│   └── chain-polkadot/
├── tools/
│   └── runtime-codegen/          # Generates codegen_runtime.rs from chain metadata
├── scripts/
│   ├── regenerate_runtimes.sh    # Regenerates all live runtime wrappers
│   └── build-containers.sh       # Docker container build
├── deployments/
│   ├── bridges/
│   │   ├── kusama-polkadot/      # Production bridge dashboards + alerts
│   │   ├── rococo-westend/       # Testnet bridge dashboards + alerts
│   │   └── rococo/               # BEEFY dashboards + alerts
│   └── local-scripts/
│       └── bridge-entrypoint.sh  # Docker entrypoint script
├── docs/                         # Additional documentation
├── Dockerfile                    # Production multi-stage Docker build
├── ci.Dockerfile                 # CI Docker image
├── RELEASE.md                    # Full release process documentation
└── deny.toml                     # cargo-deny configuration
```

## substrate-relay CLI Commands

```bash
# Initialize bridge pallet with current finalized block data
substrate-relay init-bridge <BRIDGE>

# Continuous header relay between two chains
substrate-relay relay-headers <BRIDGE> [OPTIONS]

# Relay a single header
substrate-relay relay-header <BRIDGE> [OPTIONS]

# Relay parachain heads
substrate-relay relay-parachains <BRIDGE> [OPTIONS]

# Relay single parachain head
substrate-relay relay-parachain-head <BRIDGE> [OPTIONS]

# Continuous message relay (requires header relay running)
substrate-relay relay-messages <BRIDGE> [OPTIONS]

# Relay a specific range of messages
substrate-relay relay-messages-range <BRIDGE> [OPTIONS]

# Relay delivery confirmation
substrate-relay relay-messages-delivery-confirmation <BRIDGE> [OPTIONS]

# High-level: starts 4 low-level relays (2 header + 2 message)
# Headers only relayed when needed by message relays
substrate-relay relay-headers-and-messages <BRIDGE> [OPTIONS]

# Detect and report equivocations to source chain
substrate-relay detect-equivocations <BRIDGE> [OPTIONS]
```

## CI/CD Pipeline

### CI (`ci.yml`)
Triggers on push to `master` and PRs. Jobs:
- **fmt**: `cargo +nightly fmt --all -- --check`
- **clippy**: `SKIP_WASM_BUILD=1 cargo clippy --all-targets --locked --workspace`
- **check**: `SKIP_WASM_BUILD=1 cargo check --locked --workspace`
- **test**: `cargo test --workspace` (on parity-large runner)
- **spellcheck**: hunspell on Rust files
- **deny**: cargo-deny advisories, bans, sources, licenses
- **check-rustdocs**: `cargo doc --no-deps --all --workspace --document-private-items`
- **build**: Release binary build + strip + artifact upload
- **build_docker**: Docker image build (no push on CI)

### Build & Publish (`build-tag.yml`)
Triggers on `v*` git tags. Builds and pushes Docker images to:
- `docker.io/paritytech/substrate-relay:<TAG>`
- `docker.io/paritytech/bridges-common-relay:<TAG>`

### Deploy (`deploy.yml`)
Manual workflow dispatch. Deploys to ArgoCD:
- **Server**: `argocd-chains.teleport.parity.io`
- **App**: `bridges-common-relay`
- **Packages**: `headers-a`, `headers-b`, `parachains-a`, `parachains-b`, `messages-a`, `messages-b`
- **Environment**: `parity-chains`

## Monitoring & Alerts

Existing Grafana dashboards and alert rules are defined in `deployments/bridges/`.

### Grafana Alert Files

| Bridge | File |
|--------|------|
| Kusama <> Polkadot | `deployments/bridges/kusama-polkadot/dashboard/grafana/bridge-kusama-polkadot-alerts.json` |
| Rococo <> Westend | `deployments/bridges/rococo-westend/dashboard/grafana/bridge-rococo-westend-alerts.json` |
| BEEFY (Rococo) | `deployments/bridges/rococo/dashboard/grafana/rococo-beefy-alerts.json` |

### Key Prometheus Metrics

**Finality sync** (should increase over 24h window):
- `Polkadot_to_BridgeHubKusama_Sync_best_source_at_target_block_number` (threshold: increase > 5000/24h)
- `Kusama_to_BridgeHubPolkadot_Sync_best_source_at_target_block_number` (threshold: increase > 5000/24h)
- `Rococo_to_BridgeHubWestend_Sync_best_source_at_target_block_number` (threshold: increase > 500/120m)
- `Westend_to_BridgeHubRococo_Sync_best_source_at_target_block_number` (threshold: increase > 500/390m)

**Message delivery nonces** (source_latest_generated vs target_latest_received):
- `BridgeHubPolkadot_to_BridgeHubKusama_MessageLane_00000001_lane_state_nonces`
- `BridgeHubKusama_to_BridgeHubPolkadot_MessageLane_00000001_lane_state_nonces`
- `BridgeHubRococo_to_BridgeHubWestend_MessageLane_00000002_lane_state_nonces`
- `BridgeHubWestend_to_BridgeHubRococo_MessageLane_00000002_lane_state_nonces`

**Confirmation lags** (target_latest_received - source_latest_confirmed, threshold: > 50):
- Same metric families as above with `type="source_latest_confirmed"`

**Reward lags** (source_latest_confirmed - target_latest_confirmed, threshold: > 10):
- Same metric families as above with `type="target_latest_confirmed"`

**Header mismatch / fork detection** (should be 0):
- `*_Sync_is_source_and_source_at_target_using_different_forks`
- `*_MessageLane_*_is_source_and_source_at_target_using_different_forks`

**Relay node liveness**:
- `up{container="bridges-common-relay"}` (should be 1)

**Version guard** (Loki log query):
- `{container="bridges-common-relay"} |= "Aborting relay"` (should be 0 in last 1m)

**Relay account balances** (thresholds vary):
- `at_BridgeHubKusama_relay_BridgeHubPolkadotMessages_balance` (threshold: < 2)
- `at_BridgeHubPolkadot_relay_BridgeHubKusamaMessages_balance` (threshold: < 10)
- `at_BridgeHubRococo_relay_BridgeHubWestendMessages_balance` (threshold: < 10)
- `at_BridgeHubWestend_relay_BridgeHubRococoMessages_balance` (threshold: < 10)

### Grafana Dashboards

- **Maintenance dashboards**: `kusama-polkadot-maintenance-dashboard.json`, `rococo-westend-maintenance-dashboard.json`
- **Message dashboards**: Per-direction message relay dashboards (e.g. `relay-kusama-to-polkadot-messages-dashboard.json`)
- **BEEFY**: `rococo-beefy-dashboard.json`

Dashboards are exported from: `https://grafana.teleport.parity.io/dashboards/f/eblDiw17z/Bridges`

## Release Process

See `RELEASE.md` for full details. Summary:

1. Merge all changes to `master`
2. Bump version in `substrate-relay/Cargo.toml` (current: 1.8.15)
3. Open PR with `A-release` label
4. Merge PR
5. Create annotated git tag: `git tag -a v1.X.Y -m "Release v1.X.Y"`
6. Push tag to trigger `build-tag.yml` pipeline
7. Verify Docker image on [Docker Hub](https://hub.docker.com/r/paritytech/substrate-relay/tags)
8. Create GitHub Release with bundled chain versions

### When to Release

Release when production chain runtimes change: **Polkadot**, **Kusama**, **PBH**, **KBH**. Test bridges (RBH <> WBH, RBH <> RBC) use hardcoded info and don't strictly require releases.

### Updating Chain Versions

When a chain runtime is upgraded:

1. Bump `spec_version` and `transaction_version` in the relay-client `src/lib.rs` files (see Directory Structure for paths)
2. Regenerate runtime codegen: `./scripts/regenerate_runtimes.sh`
3. Apply BlakeTwo256 workaround:
   ```bash
   cargo +nightly fmt --all
   find . -name codegen_runtime.rs -exec \
       sed -i 's/::sp_runtime::generic::Header<::core::primitive::u32>/::sp_runtime::generic::Header<::core::primitive::u32, ::sp_runtime::traits::BlakeTwo256>/g' {} +
   cargo +nightly fmt --all
   ```
4. Verify: `cargo check --workspace`

## Dependencies

- **Polkadot SDK** (master branch): Bridge primitives (`bp-*`), relay utilities, FRAME support
- **subxt** v0.40.1: Substrate extrinsic interface for runtime codegen
- **Rust**: Stable + nightly (for formatting and WASM)

## Code Review Guidelines

### Rust Code Quality
- Use `Result` types with meaningful errors; avoid `unwrap()`/`expect()` in production code
- Use `checked_*`/`saturating_*` arithmetic to prevent overflow
- Follow Rust naming conventions (snake_case for functions, CamelCase for types)

### Relay-Specific Patterns
- Chain version constants must match deployed on-chain runtime versions
- Generated `codegen_runtime.rs` files must not be manually edited
- Bridge configurations in `substrate-relay/src/bridges/` follow a consistent pattern per direction
- Transaction encoding must match the target chain's runtime version exactly

### Security
- Signer keys are passed via environment variables or CLI flags; never log or expose them
- Relay account balances must be monitored to avoid stalling
- Version guard (`"Aborting relay"`) protects against submitting incompatible transactions

### CI/CD
- All PRs must pass: fmt, clippy, check, test, deny, rustdocs
- Release PRs must have the `A-release` label
- Docker images are only pushed on tagged releases (not on CI)
