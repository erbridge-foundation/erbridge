# erbridge frontend

SvelteKit (Svelte 5, `@sveltejs/adapter-node`) UI for the E-R Bridge wormhole
mapper. It talks to the Rust/Axum backend over `/api/*` and `/auth/*`; in
development Traefik routes those prefixes to the backend and everything else
here. Internationalised with Paraglide (en/de/fr).

> **Package manager: pnpm only.** Use `pnpm` / `pnpm dlx` — never `npm` / `npx`.

## Developing

```sh
pnpm install
pnpm run dev          # start the dev server (vite dev)
pnpm run dev -- --open
```

The dev server expects the backend to be reachable; see the root
[`docker-compose.dev.yml`](../) workflow or run the backend per
[`../CONTRIBUTING.md`](../CONTRIBUTING.md).

## Building

```sh
pnpm run build        # vite build → node-adapter server in build/
pnpm run preview      # preview the production build
```

## Verification

Run all three from **this `frontend/` directory** (this repo has no pnpm
workspace root, so `pnpm --filter frontend …` fails). Each script compiles
Paraglide first, keeping `src/lib/paraglide` in sync.

```sh
pnpm test             # Vitest unit/component tests
pnpm run check        # svelte-check (type checking + Paraglide compile)
pnpm run test:e2e     # Playwright e2e tests
```

All three must pass before a change lands.

## Structure & conventions

The authoritative source for frontend structure and conventions is the
**`sveltekit-node` skill** at
[`../.claude/skills/sveltekit-node/SKILL.md`](../.claude/skills/sveltekit-node/SKILL.md)
(Svelte 5 runes, native CSS + design tokens, load functions / form actions /
server endpoints, Svelte Flow for graph UIs). The project's route and component
map lives in [`../openspec/AGENTS.md`](../openspec/AGENTS.md). Read those before
adding routes or components rather than inferring structure from the tree.
