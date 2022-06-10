#!/bin/bash
set -xeu

sleep 15

/home/user/substrate-relay init-bridge millau-to-rialto-parachain \
	--source-host millau-node-alice \
	--source-port 9944 \
	--target-host rialto-parachain-collator-alice \
	--target-port 9944 \
	--target-signer //Alice

/home/user/substrate-relay init-bridge rialto-to-millau \
	--source-host rialto-node-alice \
	--source-port 9944 \
	--target-host millau-node-alice \
	--target-port 9944 \
	--target-signer //Alice

# Give chain a little bit of time to process initialization transaction
sleep 6

/home/user/substrate-relay relay-headers-and-messages millau-rialto-parachain \
	--relayer-mode=altruistic \
	--millau-host millau-node-alice \
	--millau-port 9944 \
	--millau-signer //Charlie \
	--millau-transactions-mortality=64 \
	--rialto-parachain-host rialto-parachain-collator-charlie \
	--rialto-parachain-port 9944 \
	--rialto-parachain-signer //Charlie \
	--rialto-parachain-transactions-mortality=64 \
	--rialto-host rialto-node-alice \
	--rialto-port 9944 \
	--lane=00000000 \
	--prometheus-host=0.0.0.0
