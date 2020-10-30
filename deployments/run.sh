#!/bin/bash

# Script used for running and updating bridge deployments.
#
# To deploy a network you can run this script with the name of the network you want to run
#
# `./run.sh eth-poa-sub`
#
# To update a deployment to use the latest images available from the Docker Hub add the `update`
# argument after the bridge name.
#
# `./run.sh rialto-millau update`

set -xeu

BRIDGE=''
NETWORKS=''
case "$1" in
	eth-poa-sub)
		BRIDGE=' -f ./bridges/eth-poa-sub/docker-compose.yml '
		NETWORKS=' -f ./networks/docker-compose.rialto.yml '
		NETWORKS+=' -f ./networks/docker-compose.eth-poa.yml '
		;;
	rialto-millau)
		BRIDGE=' -f ./bridges/rialto-millau/docker-compose.yml '
		NETWORKS=' -f ./networks/docker-compose.rialto.yml '
		NETWORKS+=' -f ./networks/docker-compose.millau.yml '
		;;
	*) echo "Invalid parameter: $1 (expected eth-poa-sub/rialto-millau)"; exit 1;;
esac

MONITORING=' -f ./monitoring/docker-compose.yml '
COMPOSE_COMMAND=$BRIDGE$NETWORKS$MONITORING
BRIDGE_PATH="./bridges/$1"

# Read and source variables from .env file so we can use them here
grep -e MATRIX_ACCESS_TOKEN -e WITH_PROXY $BRIDGE_PATH/.env > .env2 && . ./.env2 && rm .env2

if [ ! -z ${MATRIX_ACCESS_TOKEN+x} ]; then
	sed -i '' -e "s/access_token.*/access_token: \"$MATRIX_ACCESS_TOKEN\"/" ./monitoring/grafana-matrix/config.yml
fi

# If we got a second argument indicating that we should update the deployment
if [ -n "${2-}" ] && [ "$2" == "update" ]; then

	# Stop the proxy cause otherwise the network can't be stopped
	cd ./reverse-proxy
	docker-compose down
	cd -

	docker-compose $COMPOSE_COMMAND pull
	docker-compose $COMPOSE_COMMAND down
	docker-compose $COMPOSE_COMMAND build
fi

if [ -n "${2-}" ] && [ "$2" == "stop" ]; then
	docker-compose --project-directory . --env-file $BRIDGE_PATH/.env $COMPOSE_COMMAND down
	exit 0
fi

# Compose looks for .env files in the the current directory by default, we don't want that
docker-compose --project-directory . --env-file $BRIDGE_PATH/.env $COMPOSE_COMMAND up -d

# Restart the proxy
if [ ! -z ${WITH_PROXY+x} ]; then
	cd ./reverse-proxy
	docker-compose up -d
fi
