#!/bin/sh
set -exu

# Make sure we are in the right dir.
cd $(dirname $(realpath $0))

# Create rialto and millau types.
jq -s '.[0] * .[1]' common.json rialto.json > ../types-rialto.json
jq -s '.[0] * .[1]' common.json millau.json > ../types-millau.json
