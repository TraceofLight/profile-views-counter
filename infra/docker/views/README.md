# views stack

Self-hosted GitHub profile views counter (komarev replacement).

## Prerequisites

The `traceoflight-edge` external Docker network must exist on the host. It is created
by the `traceoflight-api-stack` (see `traceoflight-dev/infra/docker/api/`). If you have
not started that stack yet, create the network manually:

```bash
docker network create traceoflight-edge
```

## Setup

```bash
cp .env.example .env
docker compose up -d --build
```

The container exposes port `3000` on the `traceoflight-edge` network with alias `views`.

## Wiring up the public URL

In nginx-proxy-manager, add a Custom Location to the existing `www.traceoflight.dev`
proxy host:

- Location: `/api/views`
- Scheme: `http`
- Forward Hostname: `views`
- Forward Port: `3000`

The Rust app handles both `/` and `/api/views` so no path rewriting is needed.

## Verifying

```bash
# from another container on the edge network
curl http://views:3000/health

# end-to-end after NPM is wired up
curl "https://www.traceoflight.dev/api/views?username=TraceofLight"
```

## Data

SQLite database lives in the named volume `traceoflight-views-data` at `/data/counter.db`.
Backup with `docker run --rm -v traceoflight-views-data:/d -v "$PWD":/b alpine cp /d/counter.db /b/`.
