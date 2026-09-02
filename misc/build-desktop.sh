#!/usr/bin/env bash
set -euo pipefail
cd frontend
export PUBLIC_API_HOST="$1"
encryption_pubkey=$(curl -f "https://$1/api/info" | jq -r .encryption_pubkey)
export PUBLIC_API_ENCRYPTION_PUBKEY="$encryption_pubkey"
signing_pubkey=$(curl -f "https://$1/api/info" | jq -r .signing_pubkey)
export PUBLIC_API_SIGNING_PUBKEY="$signing_pubkey"
DESKTOP=1 deno desktop --no-check --allow-env --backend cef
