# Bridge Deployments

### General notes

- Substrate authorities are named: `Alice`, `Bob`, `Charlie`, `Dave`, `Eve`, `Ferdie`.
- Ethereum authorities are named: `Arthur`, `Bertha`, `Carlos`.
- `Dockerfile`s are designed to build & run nodes & relay by fetching the sources
  from a git repo.

  You can configure commit hashes using docker build arguments:
  - `BRIDGE_REPO` - git repository of the bridge node & relay code
  - `BRIDGE_HASH` - commit hash within that repo (can also be a branch or tag)
  - `ETHEREUM_REPO` - git repository of the OpenEthereum client
  - `ETHEREUM_HASH` - commit hash within that repo (can also be a branch or tag)
  - `PROJECT` - a project to build withing bridges repo (`bridge-node` or `ethereum-poa-relay`
    currently)

  You can however uncomment `ADD` commands within the docker files to build
  an image from your local sources.

### Requirements

Make sure to install `docker` and `docker-compose` to be able to run & test
bridges deployments locally.

### Polkadot.js UI

To teach the UI decode our custom types used in the pallet, go to: `Settings -> Developer`
and import the [`./types.json`](./types.json)

## Rialto

`Rialto` is a test bridge network deployment between a test Ethereum PoA network and
test Substrate network.
Its main purpose is to make sure that basic PoA<>Substrate bridge operation works.
The network is being reset every now and then without a warning.

### Docker-Compose Deployment

To run a full network with two-way bridge functionality and cross-chain transfers you
may use the `docker-compose.yml` file in the [`rialto`](./rialto) folder. This will pull
images from the Docker Hub for all the components.

```bash
cd rialto
docker-compose pull   # Get the latest images from the Docker Hub
docker-compose build  # This is going to build images
docker-compose up     # Start all the nodes
docker-compose up -d  # Start the nodes in detached mode.
docker-compose down   # Stop the network.
```

### Docker-Compose and Git Deployment

It is also possible to avoid using images from the Docker Hub and instead build
containers from GitHub. This can be done using an override file for Docker Compose. To
build the containers you can do the following:

```bash
cd rialto
docker-compose -f docker-compose.yml -f docker-compose.git.yml build
```
The order in which you specify the compose files matters, so make sure the Git override file
comes after the base one. If you want a sanity check of the resulting compose file you may
do the following:

```bash
docker-compose -f docker-compose.yml -f docker-compose.git.yml config > docker-compose.merged.yml
```

Note that this is going to take a _very long_ time to build, since it has to build multiple
Rust projects from scratch.

When the network is running you can query logs from individual nodes using:
```bash
docker logs rialto_poa-node-bertha_1 -f
```

To kill all left over containers and start the network from scratch next time:
```bash
docker ps -a --format "{{.ID}}" | xargs docker rm # This removes all containers!
```

### Network Updates

You can update the network using the [`update.sh`](./rialto/update.sh) script. If you run it
without any arguments it will default to using the latest images from the Docker Hub. However,
you may also update the network using GitHub builds by passing the `-g` flag to the script.
There you may specify which components you want to update. You have the following options:
`all`, `node`, `relay`, and `eth`.

### Monitoring
[Prometheus](https://prometheus.io/) is used by the bridge relay to monitor information such as system
resource use, and block data (e.g the best blocks it knows about). In order to visualize this data
a [Grafana](https://grafana.com/) dashboard can be used.

As part of the Rialto `docker-compose` setup we spin up a Prometheus server and Grafana dashboard. The
Prometheus server connects to the Prometheus data endpoint exposed by the bridge relay. The Grafana
dashboard uses the Prometheus server as its data source.

The default port for the bridge relay's Prometheus data is `9616`. The host and port can be
configured though the `--prometheus-host` and `--prometheus-port` flags. The Prometheus server's
dashboard can be accessed at `http://localhost:9090`. The Grafana dashboard can be accessed at
`http://localhost:3000`. Note that the default log-in credentials for Grafana are `admin:admin`.

### UI

Use [wss://rialto.bridges.test-installations.parity.io/](https://polkadot.js.org/apps/)
as a custom endpoint for [https://polkadot.js.org/apps/](https://polkadot.js.org/apps/).

## Kovan -> Westend

???

## Scripts

The are some bash scripts in `scripts` folder that allow testing `Relay`
without running the entire network within docker. Use if needed for development.
