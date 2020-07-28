#!/bin/sh

# Script used to periodically update the network.

set -xeu

git pull
# Update BRIDGE_HASH in .env
sed -i '/BRIDGE_HASH/d' .env || true
echo "BRIDGE_HASH=$(git rev-parse HEAD)" >> .env
# Update Matrix access token
. ./.env
MATRIX_ACCESS_TOKEN=${MATRIX_ACCESS_TOKEN:-<access_token>}
sed -i "s/access_token.*/access_token: \"$MATRIX_ACCESS_TOKEN\"/" ./dashboard/grafana-matrix/config.yml

docker-compose build
# Stop the proxy cause otherwise the network can't be stopped
cd ./proxy
docker-compose down
cd -
# Restart the network
docker-compose down
docker-compose up -d

# Restart the proxy
cd ./proxy
docker-compose up -d
