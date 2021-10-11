#!/bin/bash
set -xeu

/home/user/rialto-bridge-node build-spec \
	--chain local \
	--raw \
	--disable-default-bootnode \
	> /rialto-share/rialto-relaychain-spec-raw.json

ls /rialto-share

tail -f /dev/null
