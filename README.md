# profile-views-counter

Self-hosted GitHub profile view counter, written in Rust. Drop-in replacement for
[komarev.com/ghpvc](https://komarev.com/ghpvc) with a compatible query-parameter API.

Inspired by [antonkomarev/github-profile-views-counter](https://github.com/antonkomarev/github-profile-views-counter).

## Stack

- **axum** (HTTP) + **tokio** (runtime)
- **SQLite** via `sqlx` — single file, WAL mode, persisted via Docker volume
- Single static binary, multi-stage Docker build

## API

```
GET /api/views?username=<name>&label=<text>&color=<hex>&style=<style>&abbreviated=<bool>&base=<int>
```

The same handler is also mounted at `/`, so the path can be controlled at the proxy.

| Param         | Default    | Notes                                                                |
| ------------- | ---------- | -------------------------------------------------------------------- |
| `username`    | (required) | `[A-Za-z0-9._-]{1,64}`                                               |
| `label`       | `views`    | left text                                                            |
| `color`       | `555555`   | hex (`#` optional) or named (`green`, `blue`, `red`, ...)            |
| `style`       | `flat`     | `flat`, `flat-square`, `plastic`, `for-the-badge`                    |
| `abbreviated` | `false`    | when true, formats `1234` as `1.2k`                                  |
| `base`        | `0`        | constant offset added to the count (e.g. for migrating from komarev) |

Response is `image/svg+xml` with `Cache-Control: no-store` (Camo will cache, browsers won't).

## Local dev

```bash
cargo run
# server on http://localhost:3000
```

`DATABASE_URL` defaults to `sqlite:counter.db?mode=rwc` (file in CWD).

## Deployment

See [`infra/docker/views/README.md`](./infra/docker/views/README.md). Deployment uses
Docker Compose against the existing `traceoflight-edge` network and is reverse-proxied
by nginx-proxy-manager under `www.traceoflight.dev/api/views`.
