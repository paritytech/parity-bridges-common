#!/bin/bash

# THIS SCRIPT IS NOT INTENDED FOR USE IN PRODUCTION ENVIRONMENT
#
# This scripts periodically calls the Substrate relay binary to generate messages. These messages
# are sent from the Rialto network to the Millau network.

set -eu

# Max delay before submitting transactions (s)
MAX_SUBMIT_DELAY_S=${MSG_EXCHANGE_GEN_MAX_SUBMIT_DELAY_S:-30}
MESSAGE_LANE=${MSG_EXCHANGE_GEN_LANE:-00000000}
SECONDARY_MESSAGE_LANE=${MSG_EXCHANGE_GEN_SECONDARY_LANE}
MAX_UNCONFIRMED_MESSAGES_AT_INBOUND_LANE=1024
FERDIE_ADDR=5oSLwptwgySxh5vz1HdvznQJjbQVgwYSvHEpYYeTXu1Ei8j7

SHARED_CMD="/home/user/substrate-relay send-message rialto-to-millau"
SHARED_HOST="--source-host rialto-node-bob --source-port 9944"
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

# start sending large messages immediately
LARGE_MESSAGES_TIME=0
# start sending message packs in a hour
BUNCH_OF_MESSAGES_TIME=3600

while true
do
	rand_sleep
	echo "Sending Message from Rialto to Millau"
	$SEND_MESSAGE \
		--lane $MESSAGE_LANE \
		--conversion-rate-override metric \
		raw 010109030419A8

	if [ ! -z $SECONDARY_MESSAGE_LANE ]; then
		echo "Sending Message from Rialto to Millau using secondary lane: $SECONDARY_MESSAGE_LANE"
		$SEND_MESSAGE \
			--lane $SECONDARY_MESSAGE_LANE \
			--conversion-rate-override metric \
			raw 5678
	fi

	# every other hour we're sending 3 large (size, weight, size+weight) messages
	if [ $SECONDS -ge $LARGE_MESSAGES_TIME ]; then
		LARGE_MESSAGES_TIME=$((SECONDS + 7200))

		rand_sleep
		echo "Sending Maximal Size Message from Rialto to Millau"
		$SEND_MESSAGE \
			--lane $MESSAGE_LANE \
			--conversion-rate-override metric \
			sized max
	fi

	# every other hour we're sending a bunch of small messages
	if [ $SECONDS -ge $BUNCH_OF_MESSAGES_TIME ]; then
		BUNCH_OF_MESSAGES_TIME=$((SECONDS + 7200))

		SEND_MESSAGE_OUTPUT=`$SEND_MESSAGE --lane $MESSAGE_LANE --conversion-rate-override metric raw deadbeef 2>&1`
		echo $SEND_MESSAGE_OUTPUT
		ACTUAL_CONVERSION_RATE_REGEX="conversion rate override: ([0-9\.]+)"
		if [[ $SEND_MESSAGE_OUTPUT =~ $ACTUAL_CONVERSION_RATE_REGEX ]]; then
			ACTUAL_CONVERSION_RATE=${BASH_REMATCH[1]}
		else
			echo "Unable to find conversion rate in send-message output"
			exit 1
		fi

		for i in $(seq 1 $MAX_UNCONFIRMED_MESSAGES_AT_INBOUND_LANE);
		do
			$SEND_MESSAGE \
				--lane $MESSAGE_LANE \
				--conversion-rate-override $ACTUAL_CONVERSION_RATE \
				raw deadbeef
		done

	fi
done
