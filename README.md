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
docker build -t jarvis-api:local .
docker run --rm -p 8080:8080 jarvis-api:local
curl http://localhost:8080/health
```

### GitHub → GHCR → OCI (automatic)

Workflow: [`.github/workflows/deploy-backend.yml`](.github/workflows/deploy-backend.yml)

Runs only when these paths change on `main` (or via **Actions → Deploy backend → Run workflow**):

- `apps/backend/**`
- `deploy/oci/**`
- `.github/workflows/deploy-backend.yml`

Flow: build image → push to GHCR → SCP compose → SSH `docker compose pull && up -d`.

#### 1. Repository secrets

| Secret | Required | Description |
|--------|----------|-------------|
| `OCI_HOST` | yes | Public IP / hostname of the VM |
| `OCI_USER` | yes | SSH user |
| `OCI_SSH_KEY` | yes | Private key (full PEM) |
| `OCI_PUBLIC_URL` | no | e.g. `http://1.2.3.4:8080` — enables post-deploy health check |

No pull token: the GHCR package is **public**.

#### 2. Make the GHCR package public

After the **first** successful image push:

1. GitHub → your profile/org → **Packages** → `jarvis-api`
2. **Package settings** → **Change visibility** → **Public**

Until then, `docker compose pull` on the VM will fail for anonymous pulls.

#### 3. One-time VM setup

On the OCI instance:

- Docker Engine + Compose plugin installed
- User in the `docker` group (or use root carefully)
- Security List / NSG: ingress **TCP 8080** from the internet (or your IP)
- Directory `~/jarvis` is created automatically by the workflow

CPU architecture: workflow default is `linux/arm64` (Always Free Ampere). If the VM is x86_64, set `DOCKER_PLATFORM: linux/amd64` in the workflow `env` block.

#### 4. Verify

```bash
curl http://<OCI_IP>:8080/health
# {"status":"ok"}
```

### Compose image name

[`deploy/oci/docker-compose.yml`](deploy/oci/docker-compose.yml) uses env `JARVIS_IMAGE` (set by CI), or fallback `ghcr.io/OWNER/jarvis-api:latest`. Replace `OWNER` only for manual runs; CI exports the correct image.

## Later stages

- Next.js on Vercel with Root Directory `apps/web` and a path-filtered workflow
- Multi-user accounts and client apps (Flutter / C#)
