#!/bin/sh

# Script used to periodically update the network.

set -xeu

git pull

# Update BRIDGE_HASH in .env
sed -i '/BRIDGE_HASH/d' .env || true
echo "BRIDGE_HASH=$(git rev-parse HEAD)" >> .env

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

# Make sure we grab the latest images from Docker Hub
docker-compose pull
docker-compose up -d

# Restart the proxy
if [ ! -z ${WITH_PROXY+x} ]; then
	cd ./proxy
	docker-compose up -d
fi
