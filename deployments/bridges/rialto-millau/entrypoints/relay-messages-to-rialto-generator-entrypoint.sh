#!/bin/bash

# THIS SCRIPT IS NOT INTENDED FOR USE IN PRODUCTION ENVIRONMENT
#
# This scripts periodically calls the Substrate relay binary to generate messages. These messages
# are sent from the Millau network to the Rialto network.

set -eu

# Max delay before submitting transactions (s)
MAX_SUBMIT_DELAY_S=${MSG_EXCHANGE_GEN_MAX_SUBMIT_DELAY_S:-30}
MESSAGE_LANE=${MSG_EXCHANGE_GEN_LANE:-00000000}
SECONDARY_MESSAGE_LANE=${MSG_EXCHANGE_GEN_SECONDARY_LANE}
MAX_UNCONFIRMED_MESSAGES_AT_INBOUND_LANE=128
FERDIE_ADDR=6ztG3jPnJTwgZnnYsgCDXbbQVR82M96hBZtPvkN56A9668ZC

SHARED_CMD=" /home/user/substrate-relay send-message millau-to-rialto"
SHARED_HOST="--source-host millau-node-bob --source-port 9944"
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

# last time when we have been asking for conversion rate update
LAST_CONVERSION_RATE_UPDATE_TIME=0
# current conversion rate
CONVERSION_RATE=metric

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
		CONVERSION_RATE=metric
		LAST_CONVERSION_RATE_UPDATE_TIME=$((SECONDS + 300))
	fi

	# send regular message
	echo "Sending Message from Millau to Rialto"
	SEND_MESSAGE_OUTPUT=`$SEND_MESSAGE --lane $MESSAGE_LANE --conversion-rate-override $CONVERSION_RATE raw 010109020419A8 2>&1`
	echo $SEND_MESSAGE_OUTPUT
	if [ "$CONVERSION_RATE" = "metric" ]; then
		ACTUAL_CONVERSION_RATE_REGEX="conversion rate override: ([0-9\.]+)"
		if [[ $SEND_MESSAGE_OUTPUT =~ $ACTUAL_CONVERSION_RATE_REGEX ]]; then
			CONVERSION_RATE=${BASH_REMATCH[1]}
			echo "Read updated conversion rate: $CONVERSION_RATE"
		else
			echo "Unable to find conversion rate in send-message output"
			exit 1
		fi
	fi

	if [ ! -z $SECONDARY_MESSAGE_LANE ]; then
		echo "Sending Message from Millau to Rialto using secondary lane: $SECONDARY_MESSAGE_LANE"
		$SEND_MESSAGE \
			--lane $SECONDARY_MESSAGE_LANE \
			--conversion-rate-override $CONVERSION_RATE \
			raw 5678
	fi

	# every other hour we're sending 3 large (size, weight, size+weight) messages
	if [ $SECONDS -ge $LARGE_MESSAGES_TIME ]; then
		LARGE_MESSAGES_TIME=$((SECONDS + 7200))

		rand_sleep
		echo "Sending Maximal Size Message from Millau to Rialto"
		$SEND_MESSAGE \
			--lane $MESSAGE_LANE \
			--conversion-rate-override $CONVERSION_RATE \
			sized max
	fi

	# every other hour we're sending a bunch of small messages
	if [ $SECONDS -ge $BUNCH_OF_MESSAGES_TIME ]; then
		BUNCH_OF_MESSAGES_TIME=$((SECONDS + 7200))
		for i in $(seq 0 $MAX_UNCONFIRMED_MESSAGES_AT_INBOUND_LANE);
		do
			$SEND_MESSAGE \
				--lane $MESSAGE_LANE \
				--conversion-rate-override $CONVERSION_RATE \
				raw deadbeef
		done

	fi
done
