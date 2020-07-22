#!/bin/bash
set -e

# All possible Substrate recipients (hex-encoded public keys)
SUB_RECIPIENTS=(
	# Ferdie
	"1cbd2d43530a44705ad088af313e18f80b53ef16b36177cd4b77b846f2a5f07c"
)
# All possible Ethereum signers (hex-encoded private keys)
ETH_SIGNERS=(
	# Harcoded account on OE dev chain
	"4d5db4107d237df6a3d58ee5f70ae63d73d7658d4026f2eefd2f204c81682cb7"
)
# Minimal exchange amount (in finney)
MIN_EXCHANGE_AMOUNT_FINNEY=1 # 0.1 ETH
# Maximal exchange amount (in finney)
MAX_EXCHANGE_AMOUNT_FINNEY=100000 # 100 ETH
# Max delay before submitting transactions (s)
MAX_SUBMIT_DELAY_S=60

while true
do
	# sleep some time
	SUBMIT_DELAY_S=`shuf -i 0-$MAX_SUBMIT_DELAY_S -n 1`
	echo "Sleeping $SUBMIT_DELAY_S seconds..."
	sleep $SUBMIT_DELAY_S

	# select recipient
	SUB_RECIPIENTS_MAX_INDEX=$((${#SUB_RECIPIENTS[@]} - 1))
	SUB_RECIPIENT_INDEX=`shuf -i 0-$SUB_RECIPIENTS_MAX_INDEX -n 1`
	SUB_RECIPIENT=${SUB_RECIPIENTS[$SUB_RECIPIENT_INDEX]}
	echo $SUB_RECIPIENT

	# select signer
	ETH_SIGNERS_MAX_INDEX=$((${#ETH_SIGNERS[@]} - 1))
	ETH_SIGNERS_INDEX=`shuf -i 0-$ETH_SIGNERS_MAX_INDEX -n 1`
	ETH_SIGNER=${ETH_SIGNERS[$ETH_SIGNER_INDEX]}
	echo $ETH_SIGNER

	# select amount
	EXCHANGE_AMOUNT_FINNEY=`shuf -i $MIN_EXCHANGE_AMOUNT_FINNEY-$MAX_EXCHANGE_AMOUNT_FINNEY -n 1`
	EXCHANGE_AMOUNT_ETH=`printf "%s000" $EXCHANGE_AMOUNT_FINNEY`
	echo $EXCHANGE_AMOUNT_ETH

	# submit transaction
	./ethereum-poa-relay eth-submit-exchange-tx \
		--sub-recipient=$SUB_RECIPIENT \
		--eth-signer=$ETH_SIGNER \
		--eth-amount=$EXCHANGE_AMOUNT_ETH
done
