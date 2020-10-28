# Bridge Deployments

## Requirements
Make sure to install `docker` and `docker-compose` to be able to run and test bridge deployments.

## Networks
One of the building blocks we use for our deployments are _networks_. A network is a collection of
homogenous nodes. We have Docker Compose files for each network that we want to bridge. Each of
the compose files found in the `./networks` folder is able to independently spin up a network like
so:

```bash
docker-compose -f ./networks/docker-compose.rialto.yml up
```

After running this command we would have a network of several nodes producing blocks.

## Bridges
A _bridge_ is a way for several _networks_ to connect to one another. Bridge deployments have their
own Docker Compose files which can be found in the `./bridges` folder. These Compose files typically
contain bridge relayers, which are services external to blockchain nodes, and other components such
as testing infrastructure, or front-end code.

Unlike the network Compose files, these *cannot* be deployed on their own. They must be combined
with different networks. In the following sections we'll cover how to run different bridge networks.
Note that for the following deployments we should be at the root of the `deployments` folder.

### Ethereum PoA to Rialto Substrate
In order to sync headers and transactions between an Ethereum PoA network and the Rialto network we
can use the following Compose command:

```bash
docker-compose -f ./bridges/eth-poa-sub/docker-compose.yml \
               -f ./networks/docker-compose.rialto.yml \
               -f ./networks/docker-compose.eth-poa.yml \
               -f ./monitoring/docker-compose.yml up
```

### Rialto Substrate to Millau Substrate
We can sync between the Rialto and Millau networks in a similar fashion.

```bash
docker-compose -f ./bridges/rialto-millau/docker-compose.yml \
               -f ./networks/docker-compose.rialto.yml \
               -f ./networks/docker-compose.millau.yml \
               -f ./monitoring/docker-compose.yml up
```

The two differences here are:
1. We're using the `rialto-millau` bridge compose file instead of `eth-poa-sub`
2. We're using the `millau` network compose file instead of the `eth-poa` one

### Adding Deployments
We need two main things when adding a new deployment. First, the new network which we want to
bridge. A compose file for the network should be added in the `/networks/` folder. Secondly we'll
need a new bridge compose file in `./bridges/`. This should configure the bridge relayer nodes
correctly for the two networks, and add any additional components needed for the deployment.

In general, we can deploy the bridge using `docker-compose up` in the following way:

```bash
docker-compose -f <bridge>.yml \
               -f <network_1>.yml \
               -f <network_2>.yml \
               -f <monitoring>.yml up
```

## General Notes

- Substrate authorities are named: `Alice`, `Bob`, `Charlie`, `Dave`, `Eve`, `Ferdie`.
- Ethereum authorities are named: `Arthur`, `Bertha`, `Carlos`.
- `Dockerfile`s are designed to build & run nodes & relay by fetching the sources
  from a Git repo.

  You can configure commit hashes using docker build arguments:
  - `BRIDGE_REPO` - git repository of the bridge node & relay code
  - `BRIDGE_HASH` - commit hash within that repo (can also be a branch or tag)
  - `ETHEREUM_REPO` - git repository of the OpenEthereum client
  - `ETHEREUM_HASH` - commit hash within that repo (can also be a branch or tag)
  - `PROJECT` - a project to build withing bridges repo (`rialto-bridge-node` or
    `ethereum-poa-relay` currently)

  You can however uncomment `ADD` commands within the docker files to build
  an image from your local sources.

### Docker Usage
When the network is running you can query logs from individual nodes using:
```bash
docker logs rialto_poa-node-bertha_1 -f
```

To kill all left over containers and start the network from scratch next time:
```bash
docker ps -a --format "{{.ID}}" | xargs docker rm # This removes all containers!
```

### Docker Compose Usage
If you're not familiar with how to use `docker-compose` here are some useful commands you'll need
when interacting with the bridge deployments:

```bash
docker-compose pull   # Get the latest images from the Docker Hub
docker-compose build  # This is going to build images
docker-compose up     # Start all the nodes
docker-compose up -d  # Start the nodes in detached mode.
docker-compose down   # Stop the network.
```

Note that for the you'll need to add the appropriate `-f` arguments that were mentioned in the
[Bridges](#bridges) section. You can read more about using multiple Compose files
[here](https://docs.docker.com/compose/extends/#multiple-compose-files). One thing worth noting is
that the _order_ the compose files are specified in matters. A different order will result in a
different configuration.

You can sanity check the final config like so:

```bash
docker-compose -f docker-compose.yml -f docker-compose.override.yml config > docker-compose.merged.yml
```

## Docker-Compose and Git Deployment
It is also possible to avoid using images from the Docker Hub and instead build
containers from Git. There are two ways to build the images this way.

1. Local Repo
If we want to use our local repo to build images at a particular commit we can do the following:

```bash
docker build . -f Bridge.Dockerfile -t local/<project_you're_building> --build-arg=<project>_HASH=<commit_hash>
```

This will build a local image of a particular component (can be a node or a relayer, see
[General Notes](#general-notes) for details) with a tag of `local/<project_you're_building>`. This
tag can be used in Docker Compose files.

2. GitHub Actions
We have a nightly job which runs and publishes Docker images for the different nodes and relayers to
the [ParityTech Docker Hub](https://hub.docker.com/u/paritytech) organization. These images are used
for our ephemeral (temporary) test networks. Additionally, any time a tag in the form of `v*` is
pushed to GitHub the publishing job is run. This will build all the components (nodes, relayers) and
publish them.

With images built using either method, all you have to do to use them in a deployment is change the
`image` field in the existing Docker Compose files to point to the tag of the image you want to use.

In the existing Docker Compose files you can then replace the `image` field with the images you just
built.

### Network Updates
TODO: Update this to not talk about Git updates

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
The `UI_SUBSTRATE_PROVIDER` variable lets you define the url of the Substrate node that the user interface
will connect to. `UI_ETHEREUM_PROVIDER` is used only as a guidance for users to connect
Metamask to the right Ethereum network. `UI_EXPECTED_ETHEREUM_NETWORK_ID`  is used by
the user interface as a fail safe to prevent users from connecting their Metamask extension to an
unexpected network.

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

### Polkadot.js UI

To teach the UI decode our custom types used in the pallet, go to: `Settings -> Developer`
and import the [`./types.json`](./types.json)

## Scripts

The are some bash scripts in `scripts` folder that allow testing `Relay`
without running the entire network within docker. Use if needed for development.
