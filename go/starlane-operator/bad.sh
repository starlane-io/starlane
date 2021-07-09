#!/bin/bash

echo "build and deploy"

set -e

./build.sh
./deploy.sh
