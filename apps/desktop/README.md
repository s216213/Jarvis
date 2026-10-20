# Jarvis Desktop (C#)

Placeholder — C# desktop client for Jarvis.

When this app is built, its UI must follow **future-ui-v1** (`design/future-ui-v1/tokens.json`): same navy glass, cyan/violet accents, and type roles as `apps/web`. Register the device with `platform: "desktop"`, `ui_version: "future-ui-v1"`, the app's own `client_version`, and `system_name` / `system_version` for the host OS. Show the Jarvis version as this app's UI version, `brain_version` from `GET /version`, and that response's host system version.
