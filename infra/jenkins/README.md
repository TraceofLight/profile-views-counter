# Jenkins Pipeline

Use `Pipeline script from SCM` and set the script path:

- Views job script path: `infra/jenkins/Jenkinsfile.views`

## Job Setup

1. Jenkins → New Item → name: `traceoflight-views` (or similar) → Pipeline.
2. Pipeline → Definition: `Pipeline script from SCM`.
3. SCM: Git → repo URL: `https://github.com/TraceofLight/profile-views-counter.git`.
4. Branch: `master` (or `main`).
5. Script Path: `infra/jenkins/Jenkinsfile.views`.
6. Save.

## Trigger

The pipeline is triggered by GitHub push (`triggers { githubPush() }`).
Configure a GitHub webhook on the repo pointing to `https://<jenkins-host>/github-webhook/`.

## Credentials

**No Jenkins credentials required.** The views service has no secrets — `VIEWS_PORT`
and `RUST_LOG` are checked into `.env.example` and copied at deploy time.

If you need to override values per-environment, you can later switch this to a
`Secret file` credential pattern (see `traceoflight-dev/infra/jenkins/Jenkinsfile.backend`
for the reference shape).

## Prerequisites

The `traceoflight-edge` Docker network must exist on the host. It is created by the
`traceoflight-dev` infra pipeline (`Jenkinsfile.infra` with `ACTION=apply`). If you
haven't run that yet, the views pipeline will fail at the `Verify Edge Network` stage.

## Pipeline Stages

| Stage | What it does |
| --- | --- |
| Checkout | Pulls the repo at the triggering commit |
| Prepare Env | Copies `.env.example` to `.env`, normalizes CRLF, validates required keys |
| Verify Edge Network | Fails fast if `traceoflight-edge` isn't on the host |
| Deploy Views | `docker compose ... up -d --build --no-deps views` |
| Healthcheck | Pings `http://views:3000/health` from a one-shot curl container on the edge network (max 60s) |

## Post Steps

- Removes the temporary `.env`
- Prunes stopped containers and dangling images (`docker container prune -f`, `docker image prune -f`)
- Does NOT run `docker builder prune` to avoid invalidating active build cache during concurrent jobs
