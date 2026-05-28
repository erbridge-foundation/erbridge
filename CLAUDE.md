# E-R Bridge — Project Rules for Claude

## Skill authority

Skills are the authoritative source for architecture, structure, and convention in this project. When a skill is in conflict with any other source (tasks, specs, prior code, or your own judgment), **the skill wins**.

Specific cases:

- **`rust-rest-api` skill defines the authoritative module layout for `backend/`.** Tasks and specs may name illustrative file paths; if any path conflicts with the skill's layout, follow the skill and correct the task path — do not follow the task path and do not update the skill to match.
- **`sveltekit-node` skill defines the authoritative structure for `frontend/`.** Same rule applies.

If you believe a skill rule is wrong or needs updating, **stop and raise it with the user** rather than silently working around it.

## OpenSpec change verification

For any openspec change whose implementation touches frontend code, the verification step (typically tasks.md §7 or equivalent) MUST include all three of:

- `pnpm --filter frontend test` — Vitest unit/component tests
- `pnpm --filter frontend run check` — svelte-check (type checking + paraglide compile)
- `pnpm --filter frontend run test:e2e` — Playwright e2e tests

All three must pass before a change is marked complete and before any commit lands. `pnpm test` alone is **not** sufficient — it runs only Vitest. The e2e suite catches regressions in destructive-action wiring, route changes, and form-action flows that unit tests cannot see.

When generating a change's `tasks.md`, the verification section MUST list these three commands explicitly, not "run the test suite" or similar shorthand.
