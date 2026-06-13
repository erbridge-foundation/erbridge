# E-R Bridge — Project Rules for Claude

## Scale and proportionality

This is an EVE Online wormhole-mapping tool for a small known userbase (corp/alliance scale — think tens of concurrent users, a handful of server admins), **not** a 100M-user SaaS platform. Engineer accordingly:

- **Don't build for rare edge cases at the cost of simplicity.** Prefer the straightforward solution. Reach for heavyweight machinery (extra locking, queues, sharding, elaborate retry/backoff, defensive layers for theoretical concurrency) only when a real, reachable problem at *this* scale justifies it.
- When weighing a fix, judge it against the actual blast radius and likelihood at this scale, and the cost of getting it wrong (often recoverable by hand), not against worst-case adversarial load.
- This is about **proportionality, not sloppiness** — correctness, security, and data integrity still matter. The rule trims gold-plating, not rigor. When in doubt about whether something crosses from "rigor" into "gold-plating", ask.

## Skill authority

Skills are the authoritative source for architecture, structure, and convention in this project. When a skill is in conflict with any other source (tasks, specs, prior code, or your own judgment), **the skill wins**.

Specific cases:

- **`rust-rest-api` skill defines the authoritative module layout for `backend/`.** Tasks and specs may name illustrative file paths; if any path conflicts with the skill's layout, follow the skill and correct the task path — do not follow the task path and do not update the skill to match.
- **`sveltekit-node` skill defines the authoritative structure for `frontend/`.** Same rule applies.

If you believe a skill rule is wrong or needs updating, **stop and raise it with the user** rather than silently working around it.

## OpenSpec change verification

For any openspec change whose implementation touches frontend code, the verification step (typically tasks.md §7 or equivalent) MUST include all three of:

- `pnpm test` — Vitest unit/component tests
- `pnpm run check` — svelte-check (type checking + paraglide compile)
- `pnpm run test:e2e` — Playwright e2e tests

Run these **from the `frontend/` directory**. This repo has no pnpm workspace root, so `pnpm --filter frontend …` errors with `ERR_PNPM_NO_PKG_MANIFEST` — `cd frontend` and invoke the scripts directly. (Each script chains `paraglide` compile first, so they also keep `src/lib/paraglide` in sync.)

All three must pass before a change is marked complete and before any commit lands. `pnpm test` alone is **not** sufficient — it runs only Vitest. The e2e suite catches regressions in destructive-action wiring, route changes, and form-action flows that unit tests cannot see.

When generating a change's `tasks.md`, the verification section MUST list these three commands explicitly, not "run the test suite" or similar shorthand.

## Architecture doc upkeep

`openspec/AGENTS.md` is the project's architecture/orientation map (module layout, layer relationships, router wiring, auth model, route/component trees). It exists so exploration starts from a map instead of re-deriving structure with `git grep` each time — keep it accurate.

A change MUST update `openspec/AGENTS.md` **in the same change** when it does any of:

- adds, removes, renames, or relocates a module / route / service / db / dto / component (i.e. one of the trees in that doc goes stale);
- changes a structural fact stated there (layering, auth model, router wiring, ESI/crypto notes, the i18n/locale or verification commands).

When generating a change's `tasks.md`, if the change does any of the above, the tasks MUST include an explicit step to update `openspec/AGENTS.md` (alongside the spec deltas, before the change is marked complete). Pure-behaviour changes that move no structural fact don't need to touch it. Keep edits a *map*, not a changelog — state where things are now, don't accrete history.
