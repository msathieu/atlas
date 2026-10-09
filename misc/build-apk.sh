#!/usr/bin/env bash
set -euo pipefail
cd frontend
export ANDROID_HOME="$HOME/Android"
sdkmanager --licenses
export PUBLIC_API_HOST="$1"
encryption_pubkey=$(curl -f "https://$1/api/info" | jq -r .encryption_pubkey)
export PUBLIC_API_ENCRYPTION_PUBKEY="$encryption_pubkey"
signing_pubkey=$(curl -f "https://$1/api/info" | jq -r .signing_pubkey)
export PUBLIC_API_SIGNING_PUBKEY="$signing_pubkey"
npm run build
npm exec -- cap telemetry off
if [[ ! -d android ]]; then
  npm exec -- cap add android
fi
npm exec -- cap copy
npm exec -- cap build android --keystorepath "$PWD/app.keystore" --keystorepass "$2"
