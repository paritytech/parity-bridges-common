#!/bin/bash
set -xeu

sleep 3
curl -v http://millau-node-alice:9933/health
curl -v http://rialto-node-alice:9933/health

/home/user/substrate-relay init-bridge RialtoToMillau \
	--target-host millau-node-alice \
	--target-port 9944 \
	--source-host rialto-node-alice \
	--source-port 9944 \
	--target-signer //Alice

# Give chain a little bit of time to process initialization transaction
sleep 6
/home/user/substrate-relay relay-headers RialtoToMillau \
	--target-host millau-node-alice \
	--target-port 9944 \
	--source-host rialto-node-alice \
	--source-port 9944 \
	--target-signer //Charlie \
	--prometheus-host=0.0.0.0
