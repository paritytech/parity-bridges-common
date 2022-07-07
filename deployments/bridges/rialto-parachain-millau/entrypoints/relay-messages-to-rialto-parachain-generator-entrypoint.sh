#!/bin/bash

# THIS SCRIPT IS NOT INTENDED FOR USE IN PRODUCTION ENVIRONMENT
#
# This scripts periodically calls the Substrate relay binary to generate messages. These messages
# are sent from the Millau network to the Rialto network.

set -eu

# Max delay before submitting transactions (s)
MAX_SUBMIT_DELAY_S=${MSG_EXCHANGE_GEN_MAX_SUBMIT_DELAY_S:-30}
MESSAGE_LANE=${MSG_EXCHANGE_GEN_LANE:-00000000}
MAX_UNCONFIRMED_MESSAGES_AT_INBOUND_LANE=1024

SHARED_CMD=" /home/user/substrate-relay send-message millau-to-rialto-parachain"
SHARED_HOST="--source-host millau-node-bob --source-port 9944"
SOURCE_SIGNER="--source-signer //RialtoParachain.MessagesSender"

SEND_MESSAGE="$SHARED_CMD $SHARED_HOST $SOURCE_SIGNER"

# Sleep a bit between messages
rand_sleep() {
	SUBMIT_DELAY_S=`shuf -i 0-$MAX_SUBMIT_DELAY_S -n 1`
	echo "Sleeping $SUBMIT_DELAY_S seconds..."
	sleep $SUBMIT_DELAY_S
	NOW=`date "+%Y-%m-%d %H:%M:%S"`
	echo "Woke up at $NOW"
}

# last time when we have been asking for conversion rate update
LAST_CONVERSION_RATE_UPDATE_TIME=0
# conversion rate override argument
CONVERSION_RATE_OVERRIDE="--conversion-rate-override metric"

# start sending large messages immediately
LARGE_MESSAGES_TIME=0
# start sending message packs in a hour
BUNCH_OF_MESSAGES_TIME=3600

while true
do
	rand_sleep

	# ask for latest conversion rate. We're doing that because otherwise we'll be facing
	# bans from the conversion rate provider
	if [ $SECONDS -ge $LAST_CONVERSION_RATE_UPDATE_TIME ]; then
		CONVERSION_RATE_OVERRIDE="--conversion-rate-override metric"
		CONVERSION_RATE_UPDATE_DELAY=`shuf -i 300-600 -n 1`
		LAST_CONVERSION_RATE_UPDATE_TIME=$((SECONDS + $CONVERSION_RATE_UPDATE_DELAY))
	fi

	# send regular message
	echo "Sending Message from Millau to RialtoParachain"
	SEND_MESSAGE_OUTPUT=`$SEND_MESSAGE --lane $MESSAGE_LANE $CONVERSION_RATE_OVERRIDE raw 010109020419A8 2>&1`
	echo $SEND_MESSAGE_OUTPUT
	if [ "$CONVERSION_RATE_OVERRIDE" = "--conversion-rate-override metric" ]; then
		ACTUAL_CONVERSION_RATE_REGEX="conversion rate override: ([0-9\.]+)"
		if [[ $SEND_MESSAGE_OUTPUT =~ $ACTUAL_CONVERSION_RATE_REGEX ]]; then
			CONVERSION_RATE=${BASH_REMATCH[1]}
			echo "Read updated conversion rate: $CONVERSION_RATE"
			CONVERSION_RATE_OVERRIDE="--conversion-rate-override $CONVERSION_RATE"
		else
			echo "Error: unable to find conversion rate in send-message output. Will keep using on-chain rate"
			CONVERSION_RATE_OVERRIDE=""
		fi
	fi

	# every other hour we're sending large message
	if [ $SECONDS -ge $LARGE_MESSAGES_TIME ]; then
		LARGE_MESSAGES_TIME=$((SECONDS + 7200))

		rand_sleep
		echo "Sending Maximal Size Message from RialtoParachain to Millau"
		$SEND_MESSAGE \
			--lane $MESSAGE_LANE \
			$CONVERSION_RATE_OVERRIDE \
			sized max
	fi

	# every other hour we're sending a bunch of small messages
	if [ $SECONDS -ge $BUNCH_OF_MESSAGES_TIME ]; then
		BUNCH_OF_MESSAGES_TIME=$((SECONDS + 7200))
		for i in $(seq 0 $MAX_UNCONFIRMED_MESSAGES_AT_INBOUND_LANE);
		do
			$SEND_MESSAGE \
				--lane $MESSAGE_LANE \
				$CONVERSION_RATE_OVERRIDE \
				raw 010109020419A8
		done
	fi
done
