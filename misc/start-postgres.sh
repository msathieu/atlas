#!/usr/bin/env bash
set -euo pipefail
mkdir -p postgres
podman run --rm -p 127.0.0.1:5432:5432 -v "$PWD"/postgres:/var/lib/postgresql:U -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_USER=app docker.io/library/postgres:18
