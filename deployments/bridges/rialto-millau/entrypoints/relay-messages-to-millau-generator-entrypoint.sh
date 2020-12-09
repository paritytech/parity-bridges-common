#!/bin/bash

# THIS SCRIPT IS NOT INTENDED FOR USE IN PRODUCTION ENVIRONMENT
#
# This scripts periodically calls the Substrate relay binary to generate messages. These messages
# are sent from the Rialto network to the Millau network.

set -eu

# Max delay before submitting transactions (s)
MAX_SUBMIT_DELAY_S=${MSG_EXCHANGE_GEN_MAX_SUBMIT_DELAY_S:-30}
MESSAGE_LANE=${MSG_EXCHANGE_GEN_LANE:-00000000}
FERDIE_ADDR=5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL

SHARED_CMD="/home/user/substrate-relay submit-rialto-to-millau-message"
SHARED_HOST="--rialto-host rialto-node-bob --rialto-port 9944"
DAVE_SIGNER="--rialto-signer //Dave --millau-signer //Dave"
ROOT_SIGNER="--rialto-signer //Alice --millau-signer //Alice"

SEND_MESSAGE="$SHARED_CMD $SHARED_HOST $DAVE_SIGNER"
SEND_ROOT_MESSAGE="$SHARED_CMD $SHARED_HOST $ROOT_SIGNER"

# Sleep a bit between messages
rand_sleep() {
	SUBMIT_DELAY_S=`shuf -i 0-$MAX_SUBMIT_DELAY_S -n 1`
	echo "Sleeping $SUBMIT_DELAY_S seconds..."
	sleep $SUBMIT_DELAY_S
}

while true
do
	rand_sleep
	echo "Sending Remark from Rialto to Millau using Target Origin"
	$SEND_MESSAGE \
		--lane $MESSAGE_LANE \
		--fee 100000000 \
		--origin Target \
		remark

	rand_sleep
	echo "Sending Transfer from Rialto to Millau using Target Origin"
	 $SEND_MESSAGE \
		--lane $MESSAGE_LANE \
		--fee 1000000000 \
		--origin Target \
		transfer \
		--amount 1000000000000 \
		--recipient $FERDIE_ADDR

	rand_sleep
	echo "Sending Remark from Rialto to Millau using Source Origin"
	 $SEND_MESSAGE \
		--lane $MESSAGE_LANE \
		--fee 100000000 \
		--origin Source \
		remark

	rand_sleep
	echo "Sending Transfer from Rialto to Millau using Source Origin"
	 $SEND_MESSAGE \
		--lane $MESSAGE_LANE \
		--fee 1000000000 \
		--origin Source \
		transfer \
		--amount 1000000000000 \
		--recipient $FERDIE_ADDR

	rand_sleep
	echo "Sending Remark from Rialto to Millau using Root Origin"
	 $SEND_ROOT_MESSAGE \
		--lane $MESSAGE_LANE \
		--fee 100000000 \
		--origin Root \
		remark

	rand_sleep
	echo "Sending Transfer from Rialto to Millau using Root Origin"
	 $SEND_ROOT_MESSAGE \
		--lane $MESSAGE_LANE \
		--fee 1000000000 \
		--origin Root \
		transfer \
		--amount 1000000000000 \
		--recipient $FERDIE_ADDR
done
