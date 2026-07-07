#!/usr/bin/env bash
set -euo pipefail
mkdir -p postgres
podman run --rm -p 5432:5432 -v "$PWD"/postgres:/var/lib/postgresql:U -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_USER=app docker.io/library/postgres:18 -c ssl=on -c ssl_cert_file=/etc/ssl/certs/ssl-cert-snakeoil.pem -c ssl_key_file=/etc/ssl/private/ssl-cert-snakeoil.key
