#!/bin/sh
# /game is the game folder with the probe ddraw.dll and probe.txt; mapped to C:\ostrov.
set -e
ln -sfn /game /wine/drive_c/ostrov
cd /wine/drive_c/ostrov
export WINEDLLOVERRIDES="ddraw=n,b;dsound=d"
xvfb-run -a -s "-screen 0 640x480x16" timeout "${PROBE_TIMEOUT:-300}" wine Ostrov.exe || true
wineserver -k || true
