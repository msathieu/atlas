#!/usr/bin/env bash
set -euo pipefail
podman run --rm -p 127.0.0.1:6379:6379 docker.io/valkey/valkey:9
