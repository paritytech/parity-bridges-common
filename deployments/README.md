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
_without_ the `WITH_GIT` environment variable set it will default to using the latest images from the
Docker Hub. However, you may also update the network using GitHub builds by specifying the
`WITH_GIT` environment variable. You may then pass the following options to the script in order
to only update specific components:  `all`, `node`, `relay`, and `eth`.

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

### Environment Variables

Here is an example `.env` file which is used for production deployments and network updates. For
security reasons it is not kept as part of version control. When deploying the network this
file should be correctly populated and kept in the [`rialto`](./rialto) folder.
The `UI_SUBSTRATE_PROVIDER` variable let you define the url of the nodes that the user interface
will connect to. `UI_ETHEREUM_PROVIDER` is used only as a guidance for users to connect to
Metamask to the right Ethereum network. `UI_EXPECTED_ETHEREUM_NETWORK_ID` should be set to the
Ethereum network id. This is used by the user interface to prevent users from connecting their
Metamask to an unexpected network.

```bash
GRAFANA_ADMIN_PASS=admin_pass
GRAFANA_SERVER_ROOT_URL=%(protocol)s://%(domain)s:%(http_port)s/
GRAFANA_SERVER_DOMAIN=server.domain.io
MATRIX_ACCESS_TOKEN="access-token"
WITH_GIT=1   # Optional
WITH_PROXY=1 # Optional
BRIDGE_HASH=880291a9dd3988a05b8d71cc4fd1488dea2903e1
ETH_BRIDGE_HASH=6cf4e2b5929fe5bd1b0f75aecd045b9f4ced9075
NODE_BRIDGE_HASH=00698187dcabbd6836e7b5339c03c38d1d80efed
RELAY_BRIDGE_HASH=00698187dcabbd6836e7b5339c03c38d1d80efed
UI_SUBSTRATE_PROVIDER=ws://localhost:9944
UI_ETHEREUM_PROVIDER=http://localhost:8545
UI_EXPECTED_ETHEREUM_NETWORK_ID=105
```

### UI

Use [wss://rialto.bridges.test-installations.parity.io/](https://polkadot.js.org/apps/)
as a custom endpoint for [https://polkadot.js.org/apps/](https://polkadot.js.org/apps/).

## Kovan -> Westend

???

## Scripts

The are some bash scripts in `scripts` folder that allow testing `Relay`
without running the entire network within docker. Use if needed for development.
