#!/bin/sh

# Script used to periodically update the network.

set -xeu

git pull
docker-compose build
docker-compose down
docker-compose up -d
