# Substrate-to-Substrate Bridge Relay

The bridge relay is a process that connects to running Substrate nodes and sends data over the Substrate-to-Substrate bridge. The process communicates with the nodes over the JSON-RPC interface and reads data from the relays information required by the `bridge` pallet using runtime calls and writes data to the modules by constructing and submitting extrinsics.

For more details, see the [design document](doc/design.md).

## Running in development

Run two development Substrate chains:

```bash
> TMPDIR=(mktemp -d)
> cd $TMPDIR
> substrate build-spec --dev > red-spec.json
# Modify the chain spec so that the genesis hashes of the two chains differ.
> jq '.genesis.runtime.balances.balances[0][1] |= . * 2' red-spec.json > blue-spec.json 
$ substrate --chain red-spec.json --alice --base-path ./red --port 30343 --ws-port 9954
> substrate --chain blue-spec.json --alice --base-path ./blue --port 30353 --ws-port 9964
```

Now run the bridge relay:

```
> target/release/substrate-bridge --base-path ./relay \
    --rpc-url ws://localhost:9954 \
    --rpc-url ws://localhost:9964
```
