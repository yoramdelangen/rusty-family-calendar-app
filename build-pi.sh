#!/usr/bin/env bash
set -euo pipefail

TARGET="arm-unknown-linux-gnueabihf"
REMOTE="${1:-yoram@calendar-zero.home}"
FONT="assets/fonts/zed-mono-light.ttf"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

cargo build --release --target "$TARGET"

echo "## FINISHED COMPILING"
echo "## PREPARING DEB FILE"

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/')
DEB_NAME="rusty-calendar-pi_${VERSION}_armhf.deb"
DEB_DIR="$SCRIPT_DIR/debian"

rm -rf "$DEB_DIR/usr" "$DEB_DIR/etc" "$DEB_DIR/var"

mkdir -p "$DEB_DIR/usr/local/bin"
mkdir -p "$DEB_DIR/usr/share/rusty-calendar-pi"
mkdir -p "$DEB_DIR/etc/default"
mkdir -p "$DEB_DIR/etc/systemd/system"
mkdir -p "$DEB_DIR/var/lib/rusty-calendar-pi/fonts"

cp "target/$TARGET/release/rusty-calendar-pi" "$DEB_DIR/usr/local/bin/"
cp rusty-calendar-pi.service "$DEB_DIR/etc/systemd/system/"
cp rusty-calendar-pi.env "$DEB_DIR/etc/default/rusty-calendar-pi"
cp config.toml.example "$DEB_DIR/usr/share/rusty-calendar-pi/config.toml.example"

if [ -f "$FONT" ]; then
    cp "$FONT" "$DEB_DIR/var/lib/rusty-calendar-pi/fonts/zed-mono-light.ttf"
fi

echo "## BUILDING DEB FILE"
case "$(uname -s)" in
Linux)
    dpkg-deb --build "$DEB_DIR" "$DEB_NAME"
    ;;
Darwin)
    BUILD_DIR=$(mktemp -d)
    trap 'rm -rf "$BUILD_DIR"' EXIT
    echo "2.0" > "$BUILD_DIR/debian-binary"
    tar -czf "$BUILD_DIR/control.tar.gz" --format=ustar --no-xattrs -C "$DEB_DIR/DEBIAN" .
    tar -czf "$BUILD_DIR/data.tar.gz" --format=ustar --no-xattrs -C "$DEB_DIR" usr etc var
    (cd "$BUILD_DIR" && ar rcs "$SCRIPT_DIR/$DEB_NAME" debian-binary control.tar.gz data.tar.gz)
    ;;
esac

echo "## UPLOADING...."
scp "$DEB_NAME" "$REMOTE:/tmp/$DEB_NAME"

echo "## INSTALLING...."
ssh "$REMOTE" "sudo dpkg -i /tmp/$DEB_NAME && rm -f /tmp/$DEB_NAME"

rm -f "$DEB_NAME"
rm -rf "$DEB_DIR/usr" "$DEB_DIR/etc" "$DEB_DIR/var"

echo "## FINISHED"
