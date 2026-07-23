# Rusty Calendar Pi

Lightweight calendar UI for desktop and Raspberry Pi with low CPU rendering.

## Runtime

- Desktop uses `winit` + `softbuffer`.
- Raspberry Pi uses DRM/KMS directly.
- Main flow: `src/main.rs` -> `src/layout.rs` -> `src/renderer/`.

## Storage

By default:
- debug builds use the repo root
- release builds use `~/.config/rusty-calendar-pi/`

The runtime paths can be overridden with:
- `RUSTY_CALENDAR_PI_CONFIG_DIR`
- `RUSTY_CALENDAR_PI_DATA_DIR`

Default files:
- `config.toml`
- `calendar.sqlite`

## Config

The app creates `config.toml` automatically if it does not exist.

Minimal config:

```toml
log_level = "info"

profile = []
```

`log_level` is read on startup and also shown in the footer next to the version.

Optional calendar styling:
- set `pill = true` on a profile to render that profile's events as pills by default
- set `pill = true` or `pill = false` on a calendar to override the profile setting

If no calendars are configured, the sync worker reports that as a failure instead of staying at `next sync pending` forever.

## Fonts

Fonts are not embedded in the binary.

Lookup order:
1. `RUSTY_CALENDAR_PI_FONT_DIR`
2. `$XDG_DATA_HOME/rusty-calendar-pi/fonts`
3. `~/.local/share/rusty-calendar-pi/fonts`
4. `assets/fonts`

Default requested font:
- family: `Zed Mono`
- weight: `Light`

Recommended Pi setup:
- install `zed-mono-light.ttf` into a dedicated service path and point `RUSTY_CALENDAR_PI_FONT_DIR` at it

The repo keeps `assets/fonts/` as a local drop zone, but font binaries are gitignored.

## Logging

Runtime diagnostics use `tracing`.

Typical levels:
- `error` for app failure or worker stop
- `warn` for recoverable sync failures and suspicious state
- `info` for startup and sync progress
- `debug` and `trace` for low-level diagnostics

## CLI

Available commands:
- `rusty-calendar-pi`
- `rusty-calendar-pi sync`
- `rusty-calendar-pi profile`
- `rusty-calendar-pi profile add`
- `rusty-calendar-pi calendar`
- `rusty-calendar-pi calendar add`

Interactive CLI output still goes to stdout/stderr. Runtime renderer diagnostics go through `tracing`.

## Raspberry Pi Deploy

The Pi deploy flow is installer-based.

Tracked install assets:
- `build-pi.sh`
- `install-pi.sh`
- `rusty-calendar-pi.service`
- `rusty-calendar-pi.env`
- `config.toml.example`

`build-pi.sh` now:
1. builds `arm-unknown-linux-gnueabihf`
2. uploads a small install bundle to `/tmp/rusty-calendar-pi-install`
3. optionally includes `assets/fonts/zed-mono-light.ttf`
4. runs `sudo /tmp/rusty-calendar-pi-install/install-pi.sh /tmp/rusty-calendar-pi-install`

`install-pi.sh` then:
1. creates a dedicated `rusty-calendar-pi` system user if needed
2. installs the binary to `/usr/local/bin/rusty-calendar-pi`
3. installs the service file to `/etc/systemd/system/rusty-calendar-pi.service`
4. installs the env file to `/etc/default/rusty-calendar-pi` if missing
5. installs the example config to `/etc/rusty-calendar-pi/config.toml` if missing
6. installs the font to `/var/lib/rusty-calendar-pi/fonts/zed-mono-light.ttf` when bundled
7. reloads systemd and runs `systemctl enable --now rusty-calendar-pi`

`/usr/local/bin` is the correct location for this manual install. `/usr/bin` is normally reserved for distro-managed packages.

## Systemd

Example unit file: `rusty-calendar-pi.service`

Service runtime layout:
- user: `rusty-calendar-pi`
- binary: `/usr/local/bin/rusty-calendar-pi`
- config: `/etc/rusty-calendar-pi/config.toml`
- env file: `/etc/default/rusty-calendar-pi`
- state/db: `/var/lib/rusty-calendar-pi`
- fonts: `/var/lib/rusty-calendar-pi/fonts`

The service logs to journald.

Useful commands:
- `systemctl status rusty-calendar-pi`
- `journalctl -u rusty-calendar-pi -f`

Typical setup on the Pi:

```bash
./build-pi.sh
ssh pi
sudo systemctl status rusty-calendar-pi
journalctl -u rusty-calendar-pi -f
```

## Build Notes

```bash
rustup target add arm-unknown-linux-gnueabihf
brew install arm-linux-gnueabihf-binutils
```

## Libraries

- `tiny-skia` for drawing
- `cosmic-text` for text rendering
- `taffy` for layout

## Icons

- https://github.com/carbon-design-system/carbon/tree/main/packages/icons/src/svg
