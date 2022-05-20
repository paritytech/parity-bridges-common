## Merged pull requests that need reviews

- [CheckBridgedBlockNumber signed extension to reject duplicate header-submit transactions](https://github.com/paritytech/parity-bridges-common/pull/1352);
- [Xcm in Rialto<>Millau bridge](https://github.com/paritytech/parity-bridges-common/pull/1379);
- [Parachains finality relay](https://github.com/paritytech/parity-bridges-common/pull/1199);
- [Rialto parachain <> Millau messages bridge](https://github.com/paritytech/parity-bridges-common/pull/1218).

## Code that need security audit

- the whole [parachains finality pallet](./modules/parachains);
- parts of the [bridge-runtime-common crate](./bin/runtime-common). They are likely to be removed, though;
- [CheckBridgedBlockNumber signed extension to reject duplicate header-submit transactions](https://github.com/paritytech/parity-bridges-common/pull/1352).