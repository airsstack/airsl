#!/bin/bash
# $1 = dylib: prints file(1) and the platform/version load command. The task asked for
# LC_BUILD_VERSION; run 1 showed it absent, so LC_VERSION_MIN_MACOSX is printed too.
echo "== $1"; file "$1"
echo "-- otool -l | grep -A4 LC_BUILD_VERSION:"; otool -l "$1" | grep -A4 LC_BUILD_VERSION; echo "(grep exit=$?)"
echo "-- otool -l | grep -A3 LC_VERSION_MIN_MACOSX:"; otool -l "$1" | grep -A3 LC_VERSION_MIN_MACOSX; echo "(grep exit=$?)"
