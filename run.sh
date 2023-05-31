#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

pushd server
trunk build index.html
popd

docker-compose up --build -d
