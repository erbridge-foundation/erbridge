## 1. Version source of truth

- [ ] 1.1 Freeze manifest versions: set `backend/Cargo.toml` and `frontend/package.json` to `0.0.0` with an in-file comment that the version is git-tag-derived (see RELEASING.md)
- [ ] 1.2 Write `RELEASING.md`: release = create/push `v<semver>` tag; manifest is non-authoritative; document the `0.0.0-dev.<sha>` zero-tag bootstrap and the develop `-pre.N.gSHA` format; document the **already-created** `v*` tag-protection ruleset ("Protect release tags", id 16932417 — blocks update + deletion, repo-admin bypass) and, optionally, GHCR package immutable-tags as the race-free backstop

## 2. Backend version injection

- [ ] 2.1 Extend `backend/build.rs`: when `APP_VERSION` is set and non-empty, emit `cargo:rustc-env=CARGO_PKG_VERSION=$APP_VERSION` (+ `rerun-if-env-changed=APP_VERSION`); leave `GIT_COMMIT_SHA` resolution as-is
- [ ] 2.2 Add `ARG APP_VERSION` / `ARG GIT_COMMIT_SHA` to `backend/Dockerfile` and promote to `ENV` before `cargo build --release`
- [ ] 2.3 Verify `health.rs` and `openapi.rs` need no change (both already read `CARGO_PKG_VERSION`); add/adjust a test that a non-default `APP_VERSION` flows through to `CARGO_PKG_VERSION`

## 3. Frontend version injection

- [ ] 3.1 Update `frontend/vite.config.ts`: source `PUBLIC_UI_VERSION` from `process.env.APP_VERSION` (fallback to `package.json`); add `PUBLIC_GIT_COMMIT` from `process.env.GIT_COMMIT_SHA` (fallback `"unknown"`)
- [ ] 3.2 Declare `PUBLIC_GIT_COMMIT` in `frontend/src/app.d.ts`
- [ ] 3.3 Add `ARG APP_VERSION` / `ARG GIT_COMMIT_SHA` to `frontend/Dockerfile`, promote to `ENV` before `pnpm run build`
- [ ] 3.4 Show the UI commit on `/about` (`+page.svelte`) alongside the existing UI/API version rows; update `page.svelte.test.ts`

## 4. CI version computation + build args

- [ ] 4.1 Set `fetch-depth: 0` on `actions/checkout` in both `backend.yml` and `frontend.yml` (publish job) so `git describe --tags` works
- [ ] 4.2 Add a step computing `APP_VERSION` (`git describe --tags --always --dirty`, strip leading `v`; fall back to `0.0.0-dev.<sha>` when no tag) and `GIT_COMMIT_SHA` (`git rev-parse --short HEAD`); expose as step outputs
- [ ] 4.3 Pass both as `build-args` to the `docker/build-push-action` step in each workflow
- [ ] 4.4 Add a pre-flight immutability check in each publish job: on `v*` tag pushes only, fail if `:v<semver>` already exists in GHCR (e.g. `docker manifest inspect` exit 0 → `exit 1` with a clear message); skip the check for `:develop`/`:sha-` pushes

## 5. Local build tooling

- [ ] 5.1 Update `just docker-build-backend` / `docker-build-frontend` to compute `APP_VERSION` + `GIT_COMMIT_SHA` from the local checkout and pass them as `--build-arg`

## 6. Verification

- [ ] 6.1 Build the backend image with `APP_VERSION`/`GIT_COMMIT_SHA` and confirm `GET /api/health` returns the real version + commit (not `0.1.0`/`unknown`) and OpenAPI `info.version` matches
- [ ] 6.2 Build the frontend image and confirm `/about` shows the git version + UI commit
- [ ] 6.3 Confirm `docker-compose.dev.yml` still builds and runs (it will report the documented fallbacks unless build args are added — note in RELEASING.md whether that is acceptable for dev)
- [ ] 6.4 Confirm the immutability check: a re-run/re-push of an already-published `v<semver>` fails the publish job, and the `v*` tag-protection ruleset rejects a force-update of an existing tag

## 7. Baseline release + GHCR cleanup

(Runs last — after sections 1–5 are merged to `main`, so the baseline build goes through the fixed pipeline.)

- [ ] 7.1 Cut and push the `v0.0.1` tag from `main`; confirm CI's fixed pipeline publishes `erbridge-api`/`erbridge-web` at `:v0.0.1` + `:latest` + `:sha-<short>`
- [ ] 7.2 Verify the baseline artifacts: `erbridge-api` `/api/health` reports `version: 0.0.1` + real commit; `erbridge-web` `/about` shows UI version `0.0.1` + real UI commit
- [ ] 7.3 Delete the stale pre-fix GHCR image versions (`0.1.0` / `commit unknown`) — operator runs this with a `delete:packages`-scoped token (the session token has only `repo, workflow, read:org, gist`, so it cannot); confirm only correctly-versioned artifacts remain
