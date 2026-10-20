# Jarvis

Multi-client assistant: Rust API on Oracle OCI, Next.js on Vercel, Avalonia desktop (Flutter later).

## Layout

| Path | Role |
|------|------|
| `apps/backend` | Axum API + Postgres + pula modeli free tier (Gemini, Groq) |
| `apps/web` | Next.js 16 (pnpm) — login, czat, admin |
| `apps/android` | Flutter — later |
| `apps/desktop` | Avalonia (C#) — login and chat |
| `deploy/oci` | `docker-compose.yml` + `.env.example` |

## Stage 2 — multi-user + Gemini + web

- Public: **login only** (no self-register)
- Admin creates users (`POST /admin/users` / panel `/admin`)
- First admin: `BOOTSTRAP_ADMIN_EMAIL` + `BOOTSTRAP_ADMIN_PASSWORD` when DB is empty
- Chat keeps every configured free-tier provider (Gemini and Groq when their keys are set) and picks one model per turn. The choice depends on the task, that model's quota, prompt size, and cooldowns after provider errors. A failed call falls through to the next best fit. `LLM_MODEL` pins a single model and turns task routing off.
- AI rate limits (per-user + global RPM/daily, input size, concurrency)

### Env on OCI VM

Manual file only: **`~/jarvis/.env`** (never written by CI).

```bash
# on VM
cp /path/to/repo/deploy/oci/.env.example ~/jarvis/.env
# edit secrets, GEMINI_API_KEY, GROQ_API_KEY, JWT_SECRET, POSTGRES_*, CORS_ORIGINS, bootstrap admin
```

Deploy layout:

```text
~/jarvis/
  .env                 # you maintain
  backend/             # CI overwrites
  docker-compose.yml   # CI overwrites
```

### AI limits (defaults)

| Variable | Default |
|----------|---------|
| `LLM_USER_RPM` | 5 |
| `LLM_USER_DAILY` | 50 |
| `LLM_GLOBAL_RPM` | 20 |
| `LLM_GLOBAL_DAILY` | 200 |
| `LLM_MAX_INPUT_CHARS` | 4000 |
| `LLM_CONTEXT_MESSAGES` | 20 |
| `LLM_MAX_INFLIGHT` | 3 |

Over limit → HTTP `429` `{ "error": "rate_limited", "retry_after_sec", "scope" }` without calling a provider. A provider's own 429 cools that model down and the same turn tries the next model that still fits.

### Local API (SQLite — no Docker)

Default feature is `db-sqlite`. File DB at `apps/backend/data/jarvis.db`:

```bash
cd apps/backend
cp .env.example .env   # DATABASE_URL=sqlite:./data/jarvis.db?mode=rwc
cargo run
curl http://localhost:8080/health
```

OCI / Docker image builds with `--features db-postgres` and Postgres in compose.

### Backend deploy (GitHub → build on VM)

Workflow: [`.github/workflows/deploy-backend.yml`](.github/workflows/deploy-backend.yml)

- Paths: `apps/backend/**`, `deploy/oci/**`, workflow file
- SCP sources → native `docker build` on VM → `compose up`
- Fails if `~/jarvis/.env` is missing (CI never creates it)

Secrets: `OCI_HOST`, `OCI_USER`, `OCI_SSH_KEY`, optional `OCI_PUBLIC_URL`

### Web (pnpm + Vercel)

```bash
cd apps/web
cp .env.example .env.local
# NEXT_PUBLIC_API_URL=http://localhost:8080
pnpm install
pnpm dev
```

Vercel:

- Root Directory: `apps/web`
- Install: `pnpm install`
- Build: `pnpm build`
- Env: `NEXT_PUBLIC_API_URL` → public API URL (e.g. `http://name.duckdns.org:8080`)
- Set `CORS_ORIGINS` on the API to include your Vercel domain

Package manager is pinned via `packageManager` in `apps/web/package.json` (pnpm).

### Desktop (Avalonia)

Chat, voice HUD, and listen settings. Admin tools stay in the web app. Voice turns call `POST /chat/voice`. On OCI that endpoint uses the Faster-Whisper and Kokoro containers from `deploy/oci/docker-compose.yml` (no host ports). Kokoro speaks English (`bm_george` by default); the user may speak Polish. A local `cargo run` without `SPEECH_STT_URL` and `SPEECH_TTS_URL` keeps text chat and refuses voice.

```bash
cd apps/desktop
dotnet run --project src/Jarvis.Desktop
```

.NET 10 SDK. The login screen asks for the API address (default `http://localhost:8080`).

### LLM roadmap

Jarvis keeps a pool of free-tier models and chooses one per turn. Task shape (short chat versus code, reasoning, or a long thread), each model's free-tier RPM / daily / token budget, and cooldowns after errors decide the order. One reply still comes from one model; the others stay available for the turns they fit.

1. **Now:** Gemini and Groq free tiers, routed pool (`GEMINI_API_KEY`, `GROQ_API_KEY`; optional `LLM_PROVIDERS`)
2. Later: OpenRouter free tier
3. Admin route override: `PATCH /admin/llm/route` with `provider` and `model`. `auto : auto` keeps the router.

### API sketch

| Method | Path | Auth |
|--------|------|------|
| POST | `/auth/login` | public |
| GET | `/me`, `/me/usage` | JWT |
| GET/POST | `/chat/messages` | JWT |
| POST | `/chat/voice` | JWT — WAV in, transcript plus Kokoro audio out |
| GET/POST | `/admin/users` | admin |
| PATCH/DELETE | `/admin/users/:id` | admin |
| GET | `/admin/llm/status`, `/admin/llm/models` | admin |
| PATCH | `/admin/llm/route` | admin — force `provider` and `model`, or `auto` / `auto` |
| GET | `/admin/host/metrics` | admin — OCI VM snapshot (poll every 5s) |
| POST | `/admin/host/exec` | admin — shell on the API host |
| POST | `/me/devices` | JWT — device heartbeat |
| GET/PATCH/DELETE | `/admin/devices`, `/admin/devices/:id` | admin |
| GET | `/version` | JWT — `brain_version` i wersja systemu hosta |
| GET | `/flags` | JWT |
| GET/POST | `/admin/flags` | admin |
| PATCH/DELETE | `/admin/flags/:key` | admin |

### future-ui-v1

Shared look for web, Android, and desktop. Tokens: [`design/future-ui-v1/tokens.json`](design/future-ui-v1/tokens.json). Web and the Avalonia desktop client implement them. Android follows the same tokens when that client is built.

Jarvis version is three parts: the client UI version, `brain_version` (this API, `GET /version`), and the host system version. Device registration stays separate (`ui_version`, `client_version`, `system_name`, `system_version`).

Admin tools (OCI resources, terminal, accounts, devices, feature flags) live in `apps/web` under `/admin`. VM metrics are snapshots polled every 5 seconds. On OCI, compose runs the terminal in the host namespaces (`JARVIS_HOST_EXEC=host`) with AppArmor unconfined, so `nsenter` can open `/proc/1/ns`.
