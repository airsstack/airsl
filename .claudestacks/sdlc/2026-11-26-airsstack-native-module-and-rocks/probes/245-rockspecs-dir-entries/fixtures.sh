#!/usr/bin/env bash
# Probe 245 fixtures: builds A..D under $1 (a scratch directory). real.rockspec stands for a rockspec's text.
set -euo pipefail
F=$1; rm -rf "$F"; mkdir -p "$F"/A "$F"/B "$F"/C "$F"/D "$F"/targetdir
printf 'package = "airsstack"\n' > "$F"/real.rockspec
# A: hidden entries of every kind, a directory and a FIFO under the two current names, a non-rockspec file and directory.
touch "$F"/A/.DS_Store "$F"/A/.rockspec "$F"/A/.airsstack-0.2.0-1.rockspec "$F"/A/notes.txt
mkdir "$F"/A/.hidden-dir "$F"/A/airsstack-0.2.0-1.rockspec "$F"/A/notes
mkfifo "$F"/A/airsstack-luajit-0.2.0-1.rockspec
# B: the two current names as symlinks, one to a file, one to a directory.
ln -s ../real.rockspec "$F"/B/airsstack-0.2.0-1.rockspec
ln -s ../targetdir "$F"/B/airsstack-luajit-0.2.0-1.rockspec
# C: a dangling symlink, and a regular file that is not UTF-8.
ln -s ../no-such.rockspec "$F"/C/airsstack-0.2.0-1.rockspec
printf 'package = "\xff"\n' > "$F"/C/airsstack-luajit-0.2.0-1.rockspec
# D: hidden files only.
touch "$F"/D/.DS_Store "$F"/D/.gitkeep
