# Jarvis

Multi-client assistant (Iron Man–style): Rust API on Oracle OCI, Next.js on Vercel, Flutter Android, C# desktop.

## Layout

| Path | Role |
|------|------|
| `apps/backend` | Axum API (OCI + Docker) |
| `apps/web` | Next.js (Vercel) — later |
| `apps/android` | Flutter — later |
| `apps/desktop` | C# — later |
| `deploy/oci` | `docker-compose.yml` for the VM |

## Stage 1 — backend smoke test

Minimal API:

- `GET /health` → `{"status":"ok"}`
- `GET /` → `{"service":"jarvis-api","version":"..."}`

### Local (Docker)

```bash
cd apps/backend
docker build -t jarvis-api:latest .
docker run --rm -p 8080:8080 jarvis-api:latest
curl http://localhost:8080/health
```

### GitHub → build on OCI VM (automatic)

Workflow: [`.github/workflows/deploy-backend.yml`](.github/workflows/deploy-backend.yml)

Runs only when these paths change on `main` (or via **Actions → Deploy backend → Run workflow**):

- `apps/backend/**`
- `deploy/oci/**`
- `.github/workflows/deploy-backend.yml`

Flow: Actions only uploads sources over SCP → on the VM: native `docker build` + `docker compose up`. No GHCR, no QEMU.

#### 1. Repository secrets

| Secret | Required | Description |
|--------|----------|-------------|
| `OCI_HOST` | yes | Public IP / DuckDNS hostname of the VM |
| `OCI_USER` | yes | SSH user |
| `OCI_SSH_KEY` | yes | Private key (full PEM) |
| `OCI_PUBLIC_URL` | no | e.g. `http://name.duckdns.org:8080` — post-deploy health check |

#### 2. One-time VM setup

On the OCI instance:

- Docker Engine + Compose plugin installed
- User in the `docker` group
- Security List / NSG: ingress **TCP 8080** (and SSH)
- First build downloads the Rust toolchain image and compiles deps — can take a while; later builds reuse Docker layer cache on the VM

Deploy layout on the VM (created by CI):

```text
~/jarvis/
  backend/           # sources + Dockerfile
  docker-compose.yml
```

#### 3. Verify

```bash
curl http://<host>:8080/health
# {"status":"ok"}
```

## Later stages

- Next.js on Vercel with Root Directory `apps/web` and a path-filtered workflow
- Multi-user accounts and client apps (Flutter / C#)
