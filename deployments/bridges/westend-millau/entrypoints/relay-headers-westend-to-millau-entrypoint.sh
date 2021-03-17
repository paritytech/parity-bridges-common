#!/bin/bash
set -xeu

sleep 3
curl -v http://millau-node-alice:9933/health

/home/user/substrate-relay init-bridge westend-to-millau \
	--westend-host westend-rpc.polkadot.io \
	--westend-port 443 \
	--westend-secure \
	--millau-host millau-node-alice \
	--millau-port 9944 \
	--millau-signer //George

# Give chain a little bit of time to process initialization transaction
sleep 6
/home/user/substrate-relay relay-headers westend-to-millau \
	--westend-host westend-rpc.polkadot.io \
	--westend-port 443 \
	--westend-secure \
	--millau-host millau-node-alice \
	--millau-port 9944 \
	--millau-signer //George \
	--prometheus-host=0.0.0.0
