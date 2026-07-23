Local font drop zone.

Keep font binaries out of git. Put `zed-mono-light.ttf` here for a repo-local override, or use `~/.local/share/rusty-calendar-pi/fonts/` for a machine-local install. If `XDG_DATA_HOME` is set, the app also checks `$XDG_DATA_HOME/rusty-calendar-pi/fonts`. The Pi installer places the service font in `/var/lib/rusty-calendar-pi/fonts/` and points `RUSTY_CALENDAR_PI_FONT_DIR` there.

Current default expected by the app: `Zed Mono` weight `Light`.
