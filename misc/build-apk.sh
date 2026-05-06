#!/usr/bin/env bash
set -euo pipefail
cd frontend
export PUBLIC_API_HOST="$1"
pubkey=$(curl -f "https://$1/api/info" | jq -r .pubkey)
export PUBLIC_API_PUBKEY="$pubkey"
deno task build
deno run -A npm:@capacitor/cli add android || true
deno run -A npm:@capacitor/cli copy
deno run -A npm:@capacitor/cli build android --keystorepath "$PWD/app.keystore" --keystorepass "$2"
