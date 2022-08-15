## Merged pull requests that need reviews

- [Parachains pallet benchmarks](https://github.com/paritytech/parity-bridges-common/pull/1436);
- [Use jsonrpsee subscriptions](https://github.com/paritytech/parity-bridges-common/pull/1533);
- [Unprofitable message delivery tx metric](https://github.com/paritytech/parity-bridges-common/pull/1536).

## Code that need security audit

- the whole [parachains finality pallet](./modules/parachains);
- the whole [relayers pallet](./modules/relayers);
- parts of the [bridge-runtime-common crate](./bin/runtime-common). They are likely to be removed, though;
- [CheckBridgedBlockNumber signed extension to reject duplicate header-submit transactions](https://github.com/paritytech/parity-bridges-common/pull/1352);
- [remove duplicate parachain heads exension](https://github.com/paritytech/parity-bridges-common/pull/1444);
- [Signed extension for rejecting obsolete messages pallet transactions](https://github.com/paritytech/parity-bridges-common/pull/1446).

## Code that may need security audit

- [Remove without_storage_info for messages pallet](https://github.com/paritytech/parity-bridges-common/pull/1487).