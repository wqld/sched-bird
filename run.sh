#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

pushd frontend
trunk build
popd

docker-compose up --build -d
