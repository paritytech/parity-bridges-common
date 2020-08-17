#!/bin/bash

# Script used to periodically update the network.

set -xeu

git pull

# Update *_BRIDGE_HASH in .env
function update_hash {
	echo "Updating $1"
	sed -i "/$1/d" .env || true
	echo "$1=$(git rev-parse HEAD)" >> .env
}

function process_argument {
	case "$1" in
		all) update_hash "NODE_BRIDGE_HASH"; update_hash "ETH_BRIDGE_HASH"; update_hash "RELAY_BRIDGE_HASH";;
		node) update_hash "NODE_BRIDGE_HASH";;
		relay) update_hash "RELAY_BRIDGE_HASH";;
		eth) update_hash "ETH_BRIDGE_HASH";;
		*) echo "Invalid parameter: $1 (expected all/node/relay/eth)"; exit 1;;
	esac
}
# Run at least once
process_argument ${1:-all}
shift || true
# Then process the rest.
while [[ $# -gt 0 ]]; do
	process_argument $1
	shift
done

# Update Matrix access token
grep -e MATRIX_ACCESS_TOKEN -e WITH_PROXY .env > .env2 && . ./.env2 && rm .env2

if [ ! -z ${MATRIX_ACCESS_TOKEN+x} ]; then
	sed -i "s/access_token.*/access_token: \"$MATRIX_ACCESS_TOKEN\"/" ./dashboard/grafana-matrix/config.yml
fi

# Rebuild images with latest `BRIDGE_HASH`
docker-compose build

# Stop the proxy cause otherwise the network can't be stopped
cd ./proxy
docker-compose down
cd -

# Restart the network
docker-compose down
docker-compose up -d

# Restart the proxy
if [ ! -z ${WITH_PROXY+x} ]; then
	cd ./proxy
	docker-compose up -d
fi
