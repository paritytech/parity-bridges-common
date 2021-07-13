#!/usr/bin/env bash

REGEX="error\: failed to select a version for the requirement \`(.*) = \"(.*)\"\`
candidate versions found which didn't match: (.*)
location searched"

while :
do
	OUTPUT=`cargo test --all 2>&1`
	if [[ $OUTPUT =~ $REGEX ]]
	then
		CRATE="${BASH_REMATCH[1]}"
		OLD_VERSION="${BASH_REMATCH[2]:1}"
		NEW_VERSION="${BASH_REMATCH[3]}"
		echo "Updating $CRATE:$OLD_VERSION -> $NEW_VERSION"
		sed -z "s/name = \"$CRATE\"\nversion = \"$OLD_VERSION\"/name = \"$CRATE\"\nversion = \"$NEW_VERSION\"/" -i Cargo.lock
	else
		exit
	fi
done
