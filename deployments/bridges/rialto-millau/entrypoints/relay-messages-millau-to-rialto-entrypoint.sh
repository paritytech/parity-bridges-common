#!/bin/bash
set -xeu

curl -v http://millau-node-bob:9933/health
curl -v http://rialto-node-bob:9933/health

MESSAGE_LANE=${MSG_EXCHANGE_GEN_LANE:-00000000}

/home/user/substrate-relay millau-messages-to-rialto \
	--lane $MESSAGE_LANE \
	--millau-host millau-node-bob \
	--millau-port 9944 \
	--millau-signer //Bob \
	--rialto-host rialto-node-bob \
	--rialto-port 9944 \
	--rialto-signer //Bob \
	--prometheus-host=0.0.0.0
