#!/bin/bash

# Script used to periodically update the network.
#
# Depending on whether or not the `WITH_GIT` environment variable
# is set it will update the network using images from Docker Hub
# or from GitHub.

set -xeu

# Update *_BRIDGE_HASH in .env
function update_hash {
	sed -i "/$1/d" .env || true
	echo "$1=$(git rev-parse HEAD)" >> .env
}

# Update Matrix access token
# Also source the temp environment file so we can access the variables
grep -e MATRIX_ACCESS_TOKEN -e WITH_PROXY -e WITH_GIT .env > .env2 && . ./.env2 && rm .env2

COMPOSE_EXTENSION=''
if [ ! -z ${WITH_GIT+x} ]; then
	git pull
	case "${1:-all}" in
		all) update_hash "NODE_BRIDGE_HASH"; update_hash "ETH_BRIDGE_HASH"; update_hash "RELAY_BRIDGE_HASH";;
		node) update_hash "NODE_BRIDGE_HASH";;
		relay) update_hash "RELAY_BRIDGE_HASH";;
		eth) update_hash "ETH_BRIDGE_HASH";;
		*) echo "Invalid parameter: $1 (expected all/node/relay/eth)"; exit 1;;
	esac
	COMPOSE_EXTENSION='-f docker-compose.yml -f docker-compose.git.yml'
fi

if [ ! -z ${MATRIX_ACCESS_TOKEN+x} ]; then
	sed -i "s/access_token.*/access_token: \"$MATRIX_ACCESS_TOKEN\"/" ./dashboard/grafana-matrix/config.yml
fi

if [ -z ${WITH_GIT+x} ]; then
	# Make sure we grab the latest images from Docker Hub
	docker-compose pull
fi

# Rebuild containers with latest images
docker-compose $COMPOSE_EXTENSION build

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
