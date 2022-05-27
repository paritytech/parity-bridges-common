## Merged pull requests that need reviews

- [CheckBridgedBlockNumber signed extension to reject duplicate header-submit transactions](https://github.com/paritytech/parity-bridges-common/pull/1352);
- [Complex RialtoParachain <> Millau relay](https://github.com/paritytech/parity-bridges-common/pull/1405);
- [Fixed sparse parachains finality handling in on-demand parachains relay](https://github.com/paritytech/parity-bridges-common/pull/1419).

## Code that need security audit

- the whole [parachains finality pallet](./modules/parachains);
- parts of the [bridge-runtime-common crate](./bin/runtime-common). They are likely to be removed, though;
- [CheckBridgedBlockNumber signed extension to reject duplicate header-submit transactions](https://github.com/paritytech/parity-bridges-common/pull/1352).