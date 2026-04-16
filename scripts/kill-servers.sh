#!/usr/bin/env bash
# Kill stale bellwether-web and Vite dev servers.
# Safe to run when nothing is running.
set -euo pipefail

if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" ]]; then
    taskkill //F //IM bellwether-web.exe 2>/dev/null || true
    # Kill only node processes running Vite, not all
    # node.exe instances. Uses PowerShell since wmic is
    # deprecated/removed on modern Windows 11.
    powershell -Command \
        "Get-CimInstance Win32_Process | \
         Where-Object { \$_.Name -eq 'node.exe' -and \
         \$_.CommandLine -like '*vite*' } | \
         ForEach-Object { Stop-Process -Id \$_.ProcessId \
         -Force }" 2>/dev/null || true
else
    pkill -x bellwether-web 2>/dev/null || true
    pkill -f "vite.*frontend" 2>/dev/null || true
fi

sleep 1
echo "Servers killed."
