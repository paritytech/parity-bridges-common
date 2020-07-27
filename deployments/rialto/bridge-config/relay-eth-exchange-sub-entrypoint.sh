#!/bin/bash
set -xeu

sleep 3
curl -v http://poa-node-arthur:8545/api/health
curl -v http://poa-node-bertha:8545/api/health
curl -v http://poa-node-carlos:8545/api/health
curl -v http://bridge-node-alice:9933/health
curl -v http://bridge-node-bob:9933/health
curl -v http://bridge-node-charlie:9933/health

/home/user/ethereum-poa-relay eth-exchange-sub \
	--sub-host bridge-node-alice \
	--eth-host poa-node-arthur
