## Merged pull requests that need reviews

- [CheckBridgedBlockNumber signed extension to reject duplicate header-submit transactions](https://github.com/paritytech/parity-bridges-common/pull/1352);
- [Complex RialtoParachain <> Millau relay](https://github.com/paritytech/parity-bridges-common/pull/1405);
- [Fixed sparse parachains finality handling in on-demand parachains relay](https://github.com/paritytech/parity-bridges-common/pull/1419);
- [Add RialtoParachain <> Millau bridge to test deployments](https://github.com/paritytech/parity-bridges-common/pull/1412);
- [Ensure that the bridge GRANDPA pallet is initialized in the finality relay](https://github.com/paritytech/parity-bridges-common/pull/1423);
- [Get dispatch weight from the target chain (when DispatchFeePayment::AtTargetChain is used)](https://github.com/paritytech/parity-bridges-common/pull/1430);
- [Added tracked parachains filter](https://github.com/paritytech/parity-bridges-common/pull/1432);
- [Parachains pallet benchmarks](https://github.com/paritytech/parity-bridges-common/pull/1436);
- [remove duplicate parachain heads exension](https://github.com/paritytech/parity-bridges-common/pull/1444);
- [Signed extension for rejecting obsolete messages pallet transactions](https://github.com/paritytech/parity-bridges-common/pull/1446);
- [Separate signers for different complex relay layers](https://github.com/paritytech/parity-bridges-common/pull/1465);
- [Fix on-different-forks metrics during initialization](https://github.com/paritytech/parity-bridges-common/pull/1468);
- [Store both block number and hash in best finalized storage value](https://github.com/paritytech/parity-bridges-common/pull/1475);
- [Parachain loop metrics](https://github.com/paritytech/parity-bridges-common/pull/1484);
- [Remove without_storage_info for messages pallet](https://github.com/paritytech/parity-bridges-common/pull/1487);
- [Fix and update benchmarks](https://github.com/paritytech/parity-bridges-common/pull/1494);
- [Add another condition to the reject-obsolete-parachain-heads extension](https://github.com/paritytech/parity-bridges-common/pull/1505).

## Code that need security audit

- the whole [parachains finality pallet](./modules/parachains);
- parts of the [bridge-runtime-common crate](./bin/runtime-common). They are likely to be removed, though;
- [CheckBridgedBlockNumber signed extension to reject duplicate header-submit transactions](https://github.com/paritytech/parity-bridges-common/pull/1352);
- [remove duplicate parachain heads exension](https://github.com/paritytech/parity-bridges-common/pull/1444);
- [Signed extension for rejecting obsolete messages pallet transactions](https://github.com/paritytech/parity-bridges-common/pull/1446).

## Code that may need security audit

- [Remove without_storage_info for messages pallet](https://github.com/paritytech/parity-bridges-common/pull/1487).