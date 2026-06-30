#!/usr/bin/env bash
set -euo pipefail
cd frontend
export PUBLIC_API_HOST="$1"
encryption_pubkey=$(curl -f "https://$1/api/info" | jq -r .encryption_pubkey)
export PUBLIC_API_ENCRYPTION_PUBKEY="$encryption_pubkey"
signing_pubkey=$(curl -f "https://$1/api/info" | jq -r .signing_pubkey)
export PUBLIC_API_SIGNING_PUBKEY="$signing_pubkey"
deno task build
deno run -A npm:@capacitor/cli telemetry off
deno run -A npm:@capacitor/cli add android || true
deno run -A npm:@capacitor/cli copy
deno run -A npm:@capacitor/cli build android --keystorepath "$PWD/app.keystore" --keystorepass "$2"
