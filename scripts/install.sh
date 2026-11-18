#!/bin/sh
# Installs or updates Silimon from the latest release:
#   curl -fsSL https://ndworks.dev/silimon/install.sh | sh
#
# Everything runs inside main, so a download cut short executes nothing.

set -eu

RELEASE_URL="${SILIMON_RELEASE_URL:-https://github.com/ndworks-dev/silimon/releases/latest/download}"
REQUIREMENT='anchor apple generic and identifier "dev.silimon.app" and certificate leaf[subject.OU] = "3BRKN94K4H"'

say() { printf '%s\n' "$*"; }
fail() { printf 'Silimon: %s\n' "$*" >&2; exit 1; }

fetch() {
    /usr/bin/curl --proto "${SILIMON_CURL_PROTO:-=https}" --fail --silent --show-error --location \
        --connect-timeout 15 --max-time 300 --output "$2" "$1"
}

install_dir() {
    if [ -n "${SILIMON_INSTALL_DIR:-}" ]; then
        mkdir -p "$SILIMON_INSTALL_DIR"
        printf '%s\n' "$SILIMON_INSTALL_DIR"
    elif [ -w /Applications ]; then
        printf '%s\n' /Applications
    else
        mkdir -p "$HOME/Applications"
        printf '%s\n' "$HOME/Applications"
    fi
}

main() {
    [ "$(uname -s)" = "Darwin" ] || fail "this app runs only on macOS."
    [ "$(uname -m)" = "arm64" ] || fail "this app runs only on Apple Silicon."
    major="$(sw_vers -productVersion | cut -d. -f1)"
    [ "$major" -ge 13 ] || fail "macOS 13 or later is required."

    dir="$(install_dir)"
    work="$dir/.silimon-install.$$"
    trap 'rm -rf "$work"' EXIT INT TERM
    mkdir "$work" || fail "cannot write to $dir."

    say "Downloading Silimon..."
    fetch "$RELEASE_URL/latest.json" "$work/latest.json"
    fetch "$RELEASE_URL/Silimon.zip" "$work/Silimon.zip"

    expected="$(sed -n 's/.*"sha256"[[:space:]]*:[[:space:]]*"\([0-9a-fA-F]\{64\}\)".*/\1/p' "$work/latest.json")"
    version="$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([0-9.]*\)".*/\1/p' "$work/latest.json")"
    [ -n "$expected" ] || fail "the release has no checksum."
    actual="$(/usr/bin/shasum -a 256 "$work/Silimon.zip" | cut -d' ' -f1)"
    [ "$actual" = "$expected" ] || fail "the download does not match the published checksum."

    /usr/bin/ditto -x -k "$work/Silimon.zip" "$work/unpacked"
    /usr/bin/codesign --verify --deep --strict "-R=$REQUIREMENT" "$work/unpacked/Silimon.app" 2>/dev/null \
        || fail "the app's signature was refused."

    if [ -z "${SILIMON_NO_LAUNCH:-}" ]; then
        /usr/bin/pkill -x Silimon 2>/dev/null || true
    fi
    if [ -e "$dir/Silimon.app" ]; then
        mv "$dir/Silimon.app" "$work/previous.app"
    fi
    if ! mv "$work/unpacked/Silimon.app" "$dir/Silimon.app"; then
        [ ! -e "$work/previous.app" ] || mv "$work/previous.app" "$dir/Silimon.app"
        fail "could not place the app in $dir."
    fi

    say "Silimon $version installed in $dir."
    if [ -z "${SILIMON_NO_LAUNCH:-}" ]; then
        /usr/bin/open "$dir/Silimon.app"
        say "Look for it in the menu bar."
    fi
}

main "$@"
