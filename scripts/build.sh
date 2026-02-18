#!/bin/bash
set -e

npm install --no-optional

# macOS needs electron-installer-dmg for dist
if [[ "$OSTYPE" == "darwin"* ]]; then
  npm install electron-installer-dmg
fi

npm run build-prod
