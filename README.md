# Jarvis

Multi-client assistant: Rust API on Oracle OCI, Next.js on Vercel (Flutter / C# later).

## Layout

| Path | Role |
|------|------|
| `apps/backend` | Axum API + Postgres + Gemini |
| `apps/web` | Next.js 16 (pnpm) — login, czat, admin |
| `apps/android` | Flutter — later |
| `apps/desktop` | C# — later |
| `deploy/oci` | `docker-compose.yml` + `.env.example` |

## Stage 2 — multi-user + Gemini + web

- Public: **login only** (no self-register)
- Admin creates users (`POST /admin/users` / panel `/admin`)
- First admin: `BOOTSTRAP_ADMIN_EMAIL` + `BOOTSTRAP_ADMIN_PASSWORD` when DB is empty
- Chat via Gemini; model auto-picked (latest chat-capable) unless `LLM_MODEL` is pinned
- AI rate limits (per-user + global RPM/daily, input size, concurrency)

### Env on OCI VM

Manual file only: **`~/jarvis/.env`** (never written by CI).

```bash
# on VM
cp /path/to/repo/deploy/oci/.env.example ~/jarvis/.env
# edit secrets, GEMINI_API_KEY, JWT_SECRET, POSTGRES_*, CORS_ORIGINS, bootstrap admin
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

Over limit → HTTP `429` `{ "error": "rate_limited", "retry_after_sec", "scope" }` without calling Gemini.

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

### LLM roadmap

1. **Now:** Gemini (free tier), auto model
2. Later: Groq free tier
3. Later: OpenRouter free tier
4. Later: admin UI model picker (`GET /admin/llm/models` already exists)

### API sketch

| Method | Path | Auth |
|--------|------|------|
| POST | `/auth/login` | public |
| GET | `/me`, `/me/usage` | JWT |
| GET/POST | `/chat/messages` | JWT |
| GET/POST | `/admin/users` | admin |
| PATCH/DELETE | `/admin/users/:id` | admin |
| GET | `/admin/llm/status`, `/admin/llm/models` | admin |
| GET | `/admin/host/metrics` | admin — OCI VM snapshot (poll every 5s) |
| POST | `/admin/host/exec` | admin — shell on the API host |
| POST | `/me/devices` | JWT — device heartbeat |
| GET/PATCH/DELETE | `/admin/devices`, `/admin/devices/:id` | admin |
| GET | `/version` | JWT — `brain_version` i wersja systemu hosta |
| GET | `/flags` | JWT |
| GET/POST | `/admin/flags` | admin |
| PATCH/DELETE | `/admin/flags/:key` | admin |

### future-ui-v1

Shared look for web, Android, and desktop. Tokens: [`design/future-ui-v1/tokens.json`](design/future-ui-v1/tokens.json). Web is the implementation; Android and desktop follow the same tokens when those clients are built.

Jarvis version is three parts: the client UI version, `brain_version` (this API, `GET /version`), and the host system version. Device registration stays separate (`ui_version`, `client_version`, `system_name`, `system_version`).

Admin tools (OCI resources, terminal, accounts, devices, feature flags) live in `apps/web` under `/admin`. VM metrics are snapshots polled every 5 seconds. On OCI, compose runs the terminal in the host namespaces (`JARVIS_HOST_EXEC=host`).
