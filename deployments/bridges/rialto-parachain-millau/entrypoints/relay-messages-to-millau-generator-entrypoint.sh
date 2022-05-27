#!/bin/bash

# THIS SCRIPT IS NOT INTENDED FOR USE IN PRODUCTION ENVIRONMENT
#
# This scripts periodically calls the Substrate relay binary to generate messages. These messages
# are sent from the Rialto network to the Millau network.

set -eu

# Max delay before submitting transactions (s)
MAX_SUBMIT_DELAY_S=${MSG_EXCHANGE_GEN_MAX_SUBMIT_DELAY_S:-30}
MESSAGE_LANE=${MSG_EXCHANGE_GEN_LANE:-00000000}

SHARED_CMD="/home/user/substrate-relay send-message rialto-parachain-to-millau"
SHARED_HOST="--source-host rialto-parachain-collator-bob --source-port 9944"
DAVE_SIGNER="--source-signer //Dave"

SEND_MESSAGE="$SHARED_CMD $SHARED_HOST $DAVE_SIGNER"

# Sleep a bit between messages
rand_sleep() {
	SUBMIT_DELAY_S=`shuf -i 0-$MAX_SUBMIT_DELAY_S -n 1`
	echo "Sleeping $SUBMIT_DELAY_S seconds..."
	sleep $SUBMIT_DELAY_S
	NOW=`date "+%Y-%m-%d %H:%M:%S"`
	echo "Woke up at $NOW"
}

while true
do
	rand_sleep
	echo "Sending Message from RialtoParachain to Millau"
	$SEND_MESSAGE \
		--lane $MESSAGE_LANE \
		--conversion-rate-override metric \
		raw 010109030419A8
done
