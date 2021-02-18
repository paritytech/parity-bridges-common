# How to send messages

The Substrate-to-Substrate relay comes with a command line interface (CLI) which is implemented
by the `substrate-realy` binary.

```
Substrate-to-Substrate relay

USAGE:
    substrate-relay <SUBCOMMAND>

FLAGS:
    -h, --help       
            Prints help information

    -V, --version    
            Prints version information


SUBCOMMANDS:
    help              Prints this message or the help of the given subcommand(s)
    init-bridge       Initialize on-chain bridge pallet with current header data
    relay-headers     Start headers relay between two chains
    relay-messages    Start messages relay between two chains
    send-message      Send custom message over the bridge
```
The realy related commands are basically continously running a sync loop between the `Millau` and `Rialto`
chains.
