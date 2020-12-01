#!/bin/bash

# THIS SCRIPT IS NOT INTENDED FOR USE IN PRODUCTION ENVIRONMENT
#
# This scripts periodically calls the Substrate relay binary to generate messages. These messages
# are sent from the Millau network to the Rialto network.

set -eu

# Max delay before submitting transactions (s)
MAX_SUBMIT_DELAY_S=${MSG_EXCHANGE_GEN_MAX_SUBMIT_DELAY_S:-30}
MESSAGE_LANE=${MSG_EXCHANGE_GEN_LANE:-00000000}
FERDIE_ADDR=5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL

while true
do
	# Sleep a bit between messages
	SUBMIT_DELAY_S=`shuf -i 0-$MAX_SUBMIT_DELAY_S -n 1`
	echo "Sleeping $SUBMIT_DELAY_S seconds..."
	sleep $SUBMIT_DELAY_S

	echo "Sending Remark from Millau to Rialto using Target Origin"
	/home/user/substrate-relay submit-millau-to-rialto-message \
		--millau-host millau-node-bob \
		--millau-port 9944 \
		--millau-signer //Dave \
		--rialto-signer //Dave \
		--lane $MESSAGE_LANE \
		--fee 100000000 \
		--origin Target \
		remark

	SUBMIT_DELAY_S=`shuf -i 0-$MAX_SUBMIT_DELAY_S -n 1`
	echo "Sleeping $SUBMIT_DELAY_S seconds..."
	sleep $SUBMIT_DELAY_S

	echo "Sending Transfer from Millau to Rialto using Target Origin"
	/home/user/substrate-relay submit-millau-to-rialto-message \
		--millau-host millau-node-bob \
		--millau-port 9944 \
		--millau-signer //Dave \
		--rialto-signer //Dave \
		--lane $MESSAGE_LANE \
		--fee 1000000000 \
		--origin Target \
		transfer \
		--amount 1000000000000 \
		--recipient $FERDIE_ADDR

	SUBMIT_DELAY_S=`shuf -i 0-$MAX_SUBMIT_DELAY_S -n 1`
	echo "Sleeping $SUBMIT_DELAY_S seconds..."
	sleep $SUBMIT_DELAY_S

	echo "Sending Remark from Millau to Rialto using Source Origin"
	/home/user/substrate-relay submit-millau-to-rialto-message \
		--millau-host millau-node-bob \
		--millau-port 9944 \
		--millau-signer //Dave \
		--rialto-signer //Dave \
		--lane $MESSAGE_LANE \
		--fee 100000000 \
		--origin Source \
		remark

	SUBMIT_DELAY_S=`shuf -i 0-$MAX_SUBMIT_DELAY_S -n 1`
	echo "Sleeping $SUBMIT_DELAY_S seconds..."
	sleep $SUBMIT_DELAY_S

	echo "Sending Transfer from Millau to Rialto using Source Origin"
	/home/user/substrate-relay submit-millau-to-rialto-message \
		--millau-host millau-node-bob \
		--millau-port 9944 \
		--millau-signer //Dave \
		--rialto-signer //Dave \
		--lane $MESSAGE_LANE \
		--fee 1000000000 \
		--origin Source \
		transfer \
		--amount 1000000000000 \
		--recipient $FERDIE_ADDR
done
