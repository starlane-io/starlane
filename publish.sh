#!/bin/bash
VERSION=$(cat ../VERSION)
PACKAGE=$1
SLEEP=$2
# return failure if the package is already published. (meaning this was an attempt to publish again which can instead be skipped)
cargo info --registry crates-io ${PACKAGE}@${VERSION} > /dev/null 2> /dev/null 
result=$?
if [ ${result} -eq 0 ]; then
	echo "package: ${PACKAGE}@${VERSION} has already been published... skipping."
	exit 0
else
	set -e
	cd $PACKAGE && cargo publish
	echo "publish successful... sleeping for ${SLEEP}"
	sleep $SLEEP
	exit 0
fi


