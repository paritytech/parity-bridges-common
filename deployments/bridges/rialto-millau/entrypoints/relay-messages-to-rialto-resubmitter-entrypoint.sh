#!/bin/bash
set -xeu

sleep 20
curl -v http://millau-node-alice:9933/health

# //Dave is signing Millau -> Rialto message-send transactions, which are causing problems
/home/user/substrate-relay resubmit-transactions millau \
	--target-host millau-node-alice \
	--target-port 9944 \
	--target-signer //Dave
