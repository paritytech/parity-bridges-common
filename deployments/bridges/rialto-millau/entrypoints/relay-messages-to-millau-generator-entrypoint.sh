#!/bin/bash

# THIS SCRIPT IS NOT INTENDED FOR USE IN PRODUCTION ENVIRONMENT
#
# This scripts periodically calls the Substrate relay binary to generate messages. These messages
# are sent from the Millau network to the Rialto network.

set -eu

# Max delay before submitting transactions (s)
MAX_SUBMIT_DELAY_S=${MSG_EXCHANGE_GEN_MAX_SUBMIT_DELAY_S:-60}
MESSAGE_LANE=${MSG_EXCHANGE_GEN_LANE:-00000000}
MESSAGE=${MSG_EXCHANGE_MESSAGE:-Remark}

while true
do
	# Sleep a bit between messages
	SUBMIT_DELAY_S=`shuf -i 0-$MAX_SUBMIT_DELAY_S -n 1`
	echo "Sleeping $SUBMIT_DELAY_S seconds..."
	sleep $SUBMIT_DELAY_S

	echo "Sending message from Rialto to Millau"
	/home/user/substrate-relay submit-rialto-to-millau-message \
		--rialto-host rialto-node-bob \
		--rialto-port 9944 \
		--rialto-signer //Dave \
		--millau-signer //Dave \
		--lane $MESSAGE_LANE \
		--message $MESSAGE \
		--fee 100000000
done
