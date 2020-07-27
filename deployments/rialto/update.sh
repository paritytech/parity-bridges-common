#!/bin/sh

# Script used to periodically update the network.

set -xeu

git pull
sed -i '/BRIDGE_HASH/d' .env || true
echo "BRIDGE_HASH=$(git rev-parse HEAD)" >> .env
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
