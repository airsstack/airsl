#!/bin/sh
# Installs the airsl command-line tool.
#
#   curl -LsSf https://airsstack.github.io/airsl/install.sh | sh
#   curl -LsSf https://airsstack.github.io/airsl/install.sh | AIRSL_VERSION=0.1.4 sh
#   curl -LsSf https://airsstack.github.io/airsl/install.sh | sh -s -- --no-modify-path
#
# This is a pointer, not an installer. It fetches the installer dist generated
# for one release and runs it with the arguments given here, so the short URL
# never goes stale and what it installs is exactly what that release published.
# Every variable that installer reads (AIRSL_CLI_INSTALL_DIR,
# AIRSL_CLI_NO_MODIFY_PATH, ...) passes through untouched.
#
# Published to GitHub Pages from install/ by .github/workflows/pages.yml: a
# change merged here is live for everyone who runs the command above.

set -eu

repo="https://github.com/airsstack/airsl"
installer="airsl-cli-installer.sh"

say() {
    printf 'airsl-install: %s\n' "$1" >&2
}

fail() {
    say "error: $1"
    exit 1
}

# curl first, wget otherwise: the same order dist's installer uses, so a machine
# that can run one can run the other. Both write to stdout and the shell writes
# the file, which also keeps a snap-confined curl out of the temp directory.
download() {
    if command -v curl >/dev/null 2>&1; then
        curl --proto '=https' --tlsv1.2 -sSfL "$1"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "$1"
    else
        fail "neither curl nor wget is installed"
    fi
}

main() {
    version="${AIRSL_VERSION:-latest}"
    version="${version#v}"

    case "$version" in
        latest)
            url="$repo/releases/latest/download/$installer"
            what="the latest release"
            ;;
        *[!0-9A-Za-z.+-]* | "")
            fail "AIRSL_VERSION='${AIRSL_VERSION:-}' is not a version (expected something like 0.1.4)"
            ;;
        *)
            url="$repo/releases/download/airsl-cli-v$version/$installer"
            what="airsl $version"
            ;;
    esac

    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT

    say "fetching the installer for $what"
    if ! download "$url" >"$tmp/$installer"; then
        if [ "$version" = latest ]; then
            fail "could not download $url"
        fi
        fail "could not download the installer for $what; is $version a released version? See $repo/releases"
    fi

    sh "$tmp/$installer" "$@"
}

# Nothing runs until the last line arrives, so a download cut off halfway
# through `curl | sh` defines a function and exits instead of running half a script.
main "$@"
