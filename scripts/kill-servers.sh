#!/usr/bin/env bash
# Kill stale bellwether-web processes.
# Safe to run when nothing is running.
set -euo pipefail

if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" ]]; then
    taskkill //F //IM bellwether-web.exe 2>/dev/null || true
else
    pkill -x bellwether-web 2>/dev/null || true
fi

sleep 1
echo "Servers killed."
