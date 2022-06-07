## Merged pull requests that need reviews

- [CheckBridgedBlockNumber signed extension to reject duplicate header-submit transactions](https://github.com/paritytech/parity-bridges-common/pull/1352);
- [Complex RialtoParachain <> Millau relay](https://github.com/paritytech/parity-bridges-common/pull/1405);
- [Fixed sparse parachains finality handling in on-demand parachains relay](https://github.com/paritytech/parity-bridges-common/pull/1419);
- [Add RialtoParachain <> Millau bridge to test deployments](https://github.com/paritytech/parity-bridges-common/pull/1412);
- [Ensure that the bridge GRANDPA pallet is initialized in the finality relay](https://github.com/paritytech/parity-bridges-common/pull/1423);
- [Get dispatch weight from the target chain (when DispatchFeePayment::AtTargetChain is used)](https://github.com/paritytech/parity-bridges-common/pull/1430);
- [Added tracked parachains filter](https://github.com/paritytech/parity-bridges-common/pull/1432);
- [Parachains pallet benchmarks](https://github.com/paritytech/parity-bridges-common/pull/1436).

## Code that need security audit

- the whole [parachains finality pallet](./modules/parachains);
- parts of the [bridge-runtime-common crate](./bin/runtime-common). They are likely to be removed, though;
- [CheckBridgedBlockNumber signed extension to reject duplicate header-submit transactions](https://github.com/paritytech/parity-bridges-common/pull/1352).