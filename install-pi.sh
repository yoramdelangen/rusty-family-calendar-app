#!/usr/bin/env bash
set -euo pipefail

APP="rusty-calendar-pi"
APP_USER="rusty-calendar-pi"
APP_GROUP="rusty-calendar-pi"
BUNDLE_DIR="${1:-$(pwd)}"
BIN_SRC="$BUNDLE_DIR/$APP"
SERVICE_SRC="$BUNDLE_DIR/$APP.service"
ENV_SRC="$BUNDLE_DIR/$APP.env"
CONFIG_SRC="$BUNDLE_DIR/config.toml.example"
FONT_SRC="$BUNDLE_DIR/zed-mono-light.ttf"

if [ "$(id -u)" -ne 0 ]; then
    printf '%s\n' "run as root" >&2
    exit 1
fi

if [ ! -f "$BIN_SRC" ]; then
    printf 'missing binary: %s\n' "$BIN_SRC" >&2
    exit 1
fi

if ! id -u "$APP_USER" >/dev/null 2>&1; then
    useradd --system --home-dir "/var/lib/$APP" --shell /usr/sbin/nologin --user-group "$APP_USER"
fi

for group in video render input; do
    if getent group "$group" >/dev/null 2>&1; then
        usermod -a -G "$group" "$APP_USER"
    fi
done

install -d -m 755 "/etc/$APP"
install -d -m 755 "/var/lib/$APP"
install -d -m 755 "/var/lib/$APP/fonts"

install -m 755 "$BIN_SRC" "/usr/local/bin/$APP"
install -m 644 "$SERVICE_SRC" "/etc/systemd/system/$APP.service"

if [ ! -f "/etc/default/$APP" ]; then
    install -m 644 "$ENV_SRC" "/etc/default/$APP"
fi

if [ ! -f "/etc/$APP/config.toml" ]; then
    install -m 644 "$CONFIG_SRC" "/etc/$APP/config.toml"
fi

if [ -f "$FONT_SRC" ]; then
    install -m 644 "$FONT_SRC" "/var/lib/$APP/fonts/zed-mono-light.ttf"
fi

chown -R "$APP_USER:$APP_GROUP" "/var/lib/$APP"

systemctl daemon-reload
systemctl enable --now "$APP.service"
