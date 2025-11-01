#!/usr/bin/env bash
set -euo pipefail
podman run --rm -p 6379:6379 docker.io/valkey/valkey:9
