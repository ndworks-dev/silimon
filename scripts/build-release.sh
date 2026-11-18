#!/usr/bin/env bash
# Builds the installers a Mac opens without warnings: the app signed with a
# Developer ID and notarized by Apple, as a .dmg and as the .zip that the
# install script and the app's updater fetch.
#
# Usage:
#   scripts/build-release.sh                 build, sign, notarize: dist/
#   scripts/build-release.sh --release       the same, then publish the release
#   scripts/build-release.sh --release-only  publish what is already in dist/
#
# Publishing is what offers the version to every install: the app and the
# install script read latest.json from the latest published release.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [ -f scripts/build-release.env ]; then
    # shellcheck disable=SC1091
    source scripts/build-release.env
fi

SIGN_IDENTITY="${SIGN_IDENTITY:-}"
NOTARY_PROFILE="${NOTARY_PROFILE:-}"
KEY_PATH="${APPLE_API_KEY_PATH:-}"
KEY_ID="${APPLE_API_KEY_ID:-}"
ISSUER_ID="${APPLE_API_ISSUER_ID:-}"
REPO="ndworks-dev/silimon"
DIST=dist

log() { printf '\n\033[1;34m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31mERROR:\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = "Darwin" ] || die "This builds the macOS installer and runs on a Mac."

BUILD=1
RELEASE=0
case "${1:-}" in
    "") ;;
    --release) RELEASE=1 ;;
    --release-only) RELEASE=1; BUILD=0 ;;
    *) die "Unknown argument '$1': --release, --release-only, or nothing." ;;
esac

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
[ -n "$VERSION" ] || die "No version in Cargo.toml."
TAG="v$VERSION"

if [ "$RELEASE" = 1 ]; then
    command -v gh >/dev/null || die "The GitHub CLI (gh) is not installed."
    tagged="$(git rev-list -n1 "$TAG" 2>/dev/null)" || die "There is no tag $TAG: tag the commit and push the tag first."
    [ "$tagged" = "$(git rev-parse HEAD)" ] || die "HEAD is not the commit $TAG names."
    [ -z "$(git status --porcelain)" ] || die "The working tree has changes $TAG does not have."
    git ls-remote --exit-code --tags origin "refs/tags/$TAG" >/dev/null || die "The tag $TAG has not been pushed."
    if gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1; then
        die "Release $TAG already exists."
    fi
fi

if [ "$BUILD" = 1 ]; then
    if [ -z "$SIGN_IDENTITY" ]; then
        SIGN_IDENTITY="$(security find-identity -v -p codesigning 2>/dev/null | grep 'Developer ID Application' | head -1 \
            | sed -E 's/^[[:space:]]*[0-9]+\) [A-F0-9]+ "(.*)"$/\1/')"
    fi
    [ -n "$SIGN_IDENTITY" ] || die "No 'Developer ID Application' identity in the keychain, and SIGN_IDENTITY is not set."

    if [ -n "$KEY_PATH" ] && [ -n "$KEY_ID" ] && [ -n "$ISSUER_ID" ]; then
        [ -f "$KEY_PATH" ] || die "APPLE_API_KEY_PATH names a file that is not there."
        notary=(--key "$KEY_PATH" --key-id "$KEY_ID" --issuer "$ISSUER_ID")
    elif [ -n "$NOTARY_PROFILE" ]; then
        xcrun notarytool history --keychain-profile "$NOTARY_PROFILE" >/dev/null 2>&1 \
            || die "The keychain has no notarytool profile '$NOTARY_PROFILE'."
        notary=(--keychain-profile "$NOTARY_PROFILE")
    else
        die "No notarization credentials: see scripts/build-release.env.example."
    fi

    log "Silimon $VERSION"
    cargo test --release
    SIGN_IDENTITY="$SIGN_IDENTITY" scripts/bundle.sh

    app="$DIST/Silimon.app"
    [ "$(lipo -archs "$app/Contents/MacOS/Silimon")" = "arm64" ] || die "The app is not arm64 only."

    log "Notarizing the app: this can take several minutes"
    ditto -c -k --keepParent "$app" "$DIST/notarize.zip"
    xcrun notarytool submit "$DIST/notarize.zip" "${notary[@]}" --wait
    rm -f "$DIST/notarize.zip"
    xcrun stapler staple "$app"
    xcrun stapler validate "$app"
    spctl -a -t exec -vv "$app"

    log "Building the .dmg"
    stage="$(mktemp -d)"
    trap 'rm -rf "$stage"' EXIT
    ditto "$app" "$stage/Silimon.app"
    ln -s /Applications "$stage/Applications"
    rm -f "$DIST/Silimon.dmg"
    hdiutil create -volname Silimon -srcfolder "$stage" -ov -format UDZO "$DIST/Silimon.dmg" >/dev/null
    codesign --force --timestamp --sign "$SIGN_IDENTITY" "$DIST/Silimon.dmg"

    log "Notarizing the .dmg"
    xcrun notarytool submit "$DIST/Silimon.dmg" "${notary[@]}" --wait
    xcrun stapler staple "$DIST/Silimon.dmg"
    xcrun stapler validate "$DIST/Silimon.dmg"
    spctl -a -t open --context context:primary-signature -v "$DIST/Silimon.dmg"

    rm -f "$DIST/Silimon.zip"
    ditto -c -k --keepParent "$app" "$DIST/Silimon.zip"
    sha256="$(shasum -a 256 "$DIST/Silimon.zip" | cut -d' ' -f1)"
    cat > "$DIST/latest.json" <<JSON
{
  "version": "$VERSION",
  "url": "https://github.com/$REPO/releases/download/$TAG/Silimon.zip",
  "sha256": "$sha256"
}
JSON
    log "Done: $DIST/Silimon.dmg, $DIST/Silimon.zip, $DIST/latest.json"
fi

if [ "$RELEASE" = 1 ]; then
    for file in Silimon.dmg Silimon.zip latest.json; do
        [ -f "$DIST/$file" ] || die "There is no $DIST/$file: run without --release-only to build it."
    done
    xcrun stapler validate "$DIST/Silimon.dmg" >/dev/null 2>&1 \
        || die "$DIST/Silimon.dmg carries no notarization ticket."
    grep -q "\"version\": \"$VERSION\"" "$DIST/latest.json" || die "$DIST/latest.json is not for $VERSION."

    log "Publishing $TAG"
    gh release create "$TAG" --repo "$REPO" --verify-tag --latest \
        --title "Silimon $VERSION" --generate-notes \
        "$DIST/Silimon.dmg" "$DIST/Silimon.zip" "$DIST/latest.json" scripts/install.sh
    log "Published: $(gh release view "$TAG" --repo "$REPO" --json url -q .url)"
fi
