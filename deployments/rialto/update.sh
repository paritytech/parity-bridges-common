#!/bin/bash

# Script used to periodically update the network.
#
# If used with no flags then it updates the network using Docker
# images from the Docker Hub. If used with the `-g` a.k.a the Git
# flag then it'll build the Docker containers from GitHub.

set -xeu

git_flag=false

# Update *_BRIDGE_HASH in .env
function update_hash {
	sed -i "/$1/d" .env || true
	echo "$1=$(git rev-parse HEAD)" >> .env
}

while getopts 'g:' flag; do
  case "${flag}" in
		g) git_flag=true
			case "${OPTARG}" in
				all) update_hash "NODE_BRIDGE_HASH"; update_hash "ETH_BRIDGE_HASH"; update_hash "RELAY_BRIDGE_HASH";;
				node) update_hash "NODE_BRIDGE_HASH";;
				relay) update_hash "RELAY_BRIDGE_HASH";;
				eth) update_hash "ETH_BRIDGE_HASH";;
				*) echo "Invalid parameter: $1 (expected all/node/relay/eth)"; exit 1;;
			esac
			;;
	esac
done

compose_extension=''
if [ "$git_flag" = true ]; then
	git pull
	compose_extension='-f docker-compose.yml -f docker-compose.git.yml'
fi

# Update Matrix access token
grep -e MATRIX_ACCESS_TOKEN -e WITH_PROXY .env > .env2 && . ./.env2 && rm .env2

if [ ! -z ${MATRIX_ACCESS_TOKEN+x} ]; then
	sed -i "s/access_token.*/access_token: \"$MATRIX_ACCESS_TOKEN\"/" ./dashboard/grafana-matrix/config.yml
fi

if [ "$git_flag" = false ]; then
	# Make sure we grab the latest images from Docker Hub
	docker-compose pull
fi

# Rebuild containers with latest images
docker-compose $compose_extension build

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
