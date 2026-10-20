# future-ui-v1

Visual system for every Jarvis client: `apps/web`, `apps/android`, and `apps/desktop`.

Tokens live in `tokens.json`. Web implements them in `apps/web/src/app/globals.css`. Android (Flutter) and desktop (C#) must use the same palette, type roles, radii, and glass panels when those apps are built. Do not introduce a second look.

## Look

- Near-black navy field with a faint grid and cyan/violet glow
- Glass cards, 18px radius, hairline cool borders, short HUD ticks on corners
- Display type for titles, body type for UI, mono for metrics, terminal, and flags
- Status colors: cyan (active), violet (accent), green (ok), amber (warn), rose (danger)

## Clients

When registering a device (`POST /me/devices`), send `platform` (`web`, `android`, or `desktop`) and three separate versions:

- `ui_version`: `future-ui-v1` (later UI generations replace this value)
- `client_version`: the app version, such as `0.1.0`
- `system_name` and `system_version`: the operating system and its version, such as `Windows` and `10.0.26100`

The running Jarvis instance is versioned separately:

- `ui_version`: the client UI version, such as `0.1.0`
- `brain_version`: the backend version from `GET /version`
- `system_version`: the operating system of the host that runs the backend
