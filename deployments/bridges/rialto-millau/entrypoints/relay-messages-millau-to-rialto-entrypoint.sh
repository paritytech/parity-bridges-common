#!/bin/bash
set -xeu

curl -v http://millau-node-alice:9933/health
curl -v http://rialto-node-alice:9933/health

/home/user/substrate-relay millau-messages-to-rialto \
	--lane 00000000 \
	--millau-host millau-node-alice \
	--millau-port 9933 \
	--millau-signer //Alice \
	--rialto-host rialto-node-alice \
	--rialto-port 9933 \
	--rialto-signer //Alice \
	--prometheus-host=0.0.0.0
