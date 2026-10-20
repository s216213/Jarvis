# Jarvis Desktop

Avalonia client for Jarvis. Chat, a voice HUD, and listen settings — accounts, devices, flags, host metrics, and the terminal stay in `apps/web`.

Voice uses the microphone on this machine. Push-to-talk is the default; settings can switch to a local “Jarvis” wake word when English speech recognition is installed. Audio is sent to `POST /chat/voice`. The server transcribes it and speaks the reply. Kokoro answers in English. The listen mode and Kokoro voice id are stored in the same settings file as the session.

The UI follows **future-ui-v1** (`design/future-ui-v1/tokens.json`): navy glass, cyan/violet accents, Syne / Outfit / JetBrains Mono. After login the app registers itself with `POST /me/devices`:

- `platform`: `desktop`
- `ui_version`: `future-ui-v1`
- `client_version`: this app (`0.1.0`)
- `system_name` / `system_version`: the machine running the app

The header shows that client version, `brain_version` from `GET /version`, and the host system from the same response.

## Run

```bash
cd apps/desktop
dotnet run --project src/Jarvis.Desktop
```

Requires the .NET 10 SDK. On the login screen set the API address (default `http://localhost:8080`). The address, session token, and device fingerprint are stored in `%LocalAppData%/Jarvis/desktop-settings.json`.
