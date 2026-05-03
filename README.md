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
GET /api/v1/views-counter?username=<name>&label=<text>&color=<hex>&style=<style>&abbreviated=<bool>&base=<int>
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

## Counting semantics

- **Username allowlist** — `ALLOWED_USERNAMES` (comma-separated, case-insensitive). Anything else returns a `not allowed` error badge and never touches the counter table. Default: `TraceofLight`.
- **Per-IP dedup** — repeated hits from the same client IP within a 1-hour window do not increment; the badge still renders the current count. Client IP is taken from `X-Real-IP`/`X-Forwarded-For` (set by the upstream NPM), then SHA-256(salt, ip) — only the hash is stored, never the raw IP.
- **Atomic increment** — the counter row is updated and read in a single `INSERT ... ON CONFLICT DO UPDATE ... RETURNING count`, so concurrent requests can't lose updates or read torn values.

Caveat: GitHub's Camo image proxy caches the badge, so the displayed number is "Camo refreshes + bots/unfurlers + direct hits per IP per hour", not unique humans. Same as komarev.

## Configuration

| Env var             | Default                                | Notes                                                                  |
| ------------------- | -------------------------------------- | ---------------------------------------------------------------------- |
| `DATABASE_URL`      | `sqlite:counter.db?mode=rwc`           | sqlx connection string                                                 |
| `PORT`              | `3000`                                 | bind port                                                              |
| `ALLOWED_USERNAMES` | `TraceofLight`                         | comma-separated. Empty = block everything.                             |
| `IP_HASH_SALT`      | (insecure default with `warn` log)     | random hex string. Stable within a deploy; rotates on restart is fine. |
| `RUST_LOG`          | `info`                                 | tracing-subscriber filter                                              |

## Local dev

```bash
cargo run
# server on http://localhost:3000
```

`DATABASE_URL` defaults to `sqlite:counter.db?mode=rwc` (file in CWD).

## Deployment

See [`infra/docker/views/README.md`](./infra/docker/views/README.md). Deployment uses
Docker Compose against the existing `traceoflight-edge` network and is reverse-proxied
by nginx-proxy-manager under `www.traceoflight.dev/api/v1/views-counter`.
