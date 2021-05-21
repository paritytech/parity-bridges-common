#!/bin/sh

# The script generates JSON type definition files in `./deployment` directory to be used for
# JS clients.
# Both networks have a lot of common types, so to avoid duplication we merge `common.json` file with
# chain-specific definitions in `rialto|millau.json`.

set -eux

# Make sure we are in the right dir.
cd $(dirname $(realpath $0))

# Create types for our supported bridge pairs (Rialto<>Millau, Rococo<>Wococo)
jq -s '.[0] * .[1]' common.json rialto.json > ../types-rialto.json
jq -s '.[0] * .[1]' common.json millau.json > ../types-millau.json
jq -s '.[0] * .[1]' common.json rococo.json > ../types-rococo.json
jq -s '.[0] * .[1]' common.json wococo.json > ../types-wococo.json
