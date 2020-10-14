#!/bin/bash
set -xeu

sleep 3
curl -v http://millau-bridge-node-alice:9933/health
curl -v http://millau-bridge-node-bob:9933/health
curl -v http://millau-bridge-node-charlie:9933/health
curl -v http://millau-bridge-node-dave:9933/health
curl -v http://millau-bridge-node-eve:9933/health

/home/user/substrate-relay millau-headers-to-rialto \
	--millau-host millau-bridge-node-alice \
	--millau-port 9944 \
	--rialto-host rialto-bridge-node-alice \
	--rialto-port 9944 \
	--rialto-signer //Alice \
	--prometheus-host=0.0.0.0
