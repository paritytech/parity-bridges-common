#!/bin/sh

# One-liner to update between Substrate `rc` releases
# Usage: ./update_rc.sh rc4 rc5

OLD_RC_VERSION=$1
NEW_RC_VERSION=$2

find . -type f -name 'Cargo.toml' -exec sed -i '' -e "s/$OLD_RC_VERSION/$NEW_RC_VERSION/g" {} \;
