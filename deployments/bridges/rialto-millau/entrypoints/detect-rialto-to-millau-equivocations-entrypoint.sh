#!/bin/bash
set -eu

exec /home/user/substrate-relay detect-equivocations rialto-to-millau \
	--source-host rialto-node-alice \
	--source-port 9944 \
	--source-signer //Millau.HeadersAndMessagesRelay \
	--source-transactions-mortality=64 \
  --target-host millau-node-alice \
  --target-port 9944
