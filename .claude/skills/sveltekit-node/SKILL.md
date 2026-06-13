---
name: sveltekit-node
description: |
  Rules for the SvelteKit frontend: node adapter, Svelte 5 runes, native CSS
  (no Tailwind, no CSS-in-JS), Svelte Flow for graph UIs, pages/layouts/load
  functions/form actions/server endpoints, and the project's design-token system.
  TRIGGER when: starting work on any task in the `frontend/` directory of this
  repo, including the first scaffolding tasks before files exist; editing any
  file whose path contains `frontend/` (especially `frontend/src/routes/`,
  `frontend/src/lib/`, `frontend/src/app.css`, `frontend/svelte.config.js`);
  writing or modifying `+page.svelte`, `+page.server.ts`, `+layout.svelte`,
  `+layout.server.ts`, `+server.ts`, or form actions; touching Svelte 5 runes
  (`$state`, `$derived`, `$props`, `$effect`); adding or modifying Svelte Flow
  canvases; reviewing a frontend PR; applying tasks from an OpenSpec change
  whose tasks.md mentions frontend files. Invoke before writing the first line
  of Svelte / TypeScript in `frontend/` in a session.
  SKIP: backend Rust code, infrastructure-only changes (Dockerfiles, Compose,
  Traefik config), or documentation changes that don't touch frontend code.
---

# SvelteKit (Node Adapter) — Rules & Guidance

## Stack Constraints

| Concern | Choice |
|---|---|
| Framework | SvelteKit, latest stable |
| Adapter | `@sveltejs/adapter-node` |
| Svelte version | **Svelte 5** — runes only (see below) |
| Styling | **Native CSS** — no Tailwind, no CSS-in-JS, no utility class frameworks |
| Graph / flow UI | **Svelte Flow** (`@xyflow/svelte`) |
| TypeScript | Required everywhere |

---

## Svelte 5 — Runes Only

Svelte 5 introduces runes. Use them exclusively. **Do not use Svelte 4 reactivity APIs** (`$:`, `export let`, writable stores for local state, `onMount` for reactive data).

### State

```svelte
<!-- CORRECT -->
<script lang="ts">
  let count = $state(0);
  let doubled = $derived(count * 2);
</script>

<!-- WRONG -->
<script lang="ts">
  let count = 0;          // ❌ not reactive
  $: doubled = count * 2; // ❌ Svelte 4 reactive statement
</script>
```

### Props

```svelte
<!-- CORRECT -->
<script lang="ts">
  let { label, onClick }: { label: string; onClick: () => void } = $props();
</script>

<!-- WRONG -->
<script lang="ts">
  export let label: string; // ❌ Svelte 4 prop syntax
</script>
```

### Effects

```svelte
<script lang="ts">
  // CORRECT — runs when deps change
  $effect(() => {
    console.log('count changed', count);
  });

  // Use $effect.pre for DOM-before-update work
  $effect.pre(() => { /* … */ });
</script>
```

### Shared / global state

Use `$state` inside a `.svelte.ts` module (not a plain `.ts` file) for shared reactive state. Do **not** use Svelte stores for local or shared component state — that's what runes are for. Stores are still fine for third-party library integration when required.

```ts
// src/lib/state/cart.svelte.ts
export const cart = $state({ items: [] as CartItem[] });
```

### Snippets (replace slots)

```svelte
<!-- CORRECT — Svelte 5 snippets -->
{#snippet header()}
  <h1>Title</h1>
{/snippet}

{@render header()}

<!-- WRONG — Svelte 4 slots -->
<slot name="header" />  <!-- ❌ -->
```

---

## Project Structure

This skill applies to everything under `<projectroot>/frontend/`. The SvelteKit
project root is `frontend/` — all paths below are relative to it.

```
src/
├── lib/
│   ├── components/        # Shared UI components (.svelte)
│   ├── state/             # Shared rune state (.svelte.ts)
│   ├── server/            # Server-only utilities (DB clients, etc.)
│   │   └── db.ts
│   ├── types.ts           # Shared TypeScript types / interfaces
│   └── utils.ts           # Pure utility functions
├── routes/
│   ├── +layout.svelte
│   ├── +layout.ts         # Root load function (session, common data)
│   ├── (app)/             # Route group — authenticated routes
│   │   └── dashboard/
│   │       ├── +page.svelte
│   │       └── +page.server.ts
│   └── api/
│       └── [...resource]/
│           └── +server.ts
└── app.css                # Global CSS (custom properties, resets, base)
```

Rules:
- `src/lib/server/` is **never imported by client code** — SvelteKit enforces this; respect it.
- Route groups `(name)/` for logical grouping without affecting the URL.
- Co-locate `+page.server.ts` (server load + actions) with its `+page.svelte`.

---

## Data Loading

### `+page.server.ts` — server load (preferred for data fetching)

```ts
// src/routes/dashboard/+page.server.ts
import type { PageServerLoad } from './$types';
import { db } from '$lib/server/db';
import { error } from '@sveltejs/kit';

export const load: PageServerLoad = async ({ locals }) => {
  const user = locals.user;
  if (!user) error(401, 'Unauthorised');

  const projects = await db.projects.listByUser(user.id);
  return { projects }; // typed — consumed as `data` in +page.svelte
};
```

### `+page.ts` — universal load (runs on server + client)

Only use universal load when the data is public and cache-friendly, or when you need it to re-run on client navigation without a full server round-trip.

### Never fetch from `+page.svelte`

Components do not call `fetch` directly for initial data. All initial data comes from `load()`. User-triggered mutations go through form actions or `+server.ts` endpoints.

---

## Form Actions

Prefer form actions over API routes for mutations triggered by user interaction.

```ts
// +page.server.ts
import type { Actions } from './$types';
import { fail, redirect } from '@sveltejs/kit';

export const actions: Actions = {
  create: async ({ request, locals }) => {
    const data = await request.formData();
    const name = data.get('name');

    if (!name || typeof name !== 'string') {
      return fail(422, { error: 'Name is required' });
    }

    await db.projects.create({ name, userId: locals.user.id });
    redirect(303, '/dashboard');
  },
};
```

```svelte
<!-- +page.svelte -->
<script lang="ts">
  import { enhance } from '$app/forms';
  let { form } = $props(); // typed as ActionData
</script>

<form method="POST" action="?/create" use:enhance>
  <input name="name" />
  {#if form?.error}<p>{form.error}</p>{/if}
  <button type="submit">Create</button>
</form>
```

Use `use:enhance` on all forms to get progressive enhancement without a full page reload.

---

## API Routes (`+server.ts`)

Use for:
- Non-form AJAX mutations (e.g., drag-and-drop saves, real-time updates)
- Endpoints consumed by external clients
- Webhooks

```ts
// src/routes/api/projects/[id]/+server.ts
import type { RequestHandler } from './$types';
import { json, error } from '@sveltejs/kit';

export const PATCH: RequestHandler = async ({ params, request, locals }) => {
  if (!locals.user) error(401);

  const body = await request.json();
  const updated = await db.projects.update(params.id, body);
  return json(updated);
};
```

Wrap responses in the same `{ data: … }` envelope used by the backend where it makes sense for API consumers. For internal SvelteKit use (load functions calling own endpoints), prefer server load functions instead.

---

## Native CSS Rules

- **One stylesheet hierarchy**: `src/app.css` for custom properties, resets, and base styles. Component styles in `<style>` blocks (scoped by default).
- **Custom properties for all design tokens** — colours, spacing, radii, type scale. No magic numbers inline.
  - **Blessed exception — translucent overlays.** Backdrop scrims, box-shadows, and tinted notice backgrounds may use literal alpha colours (e.g. `rgba(0, 0, 0, 0.6)`, `rgba(245, 158, 11, 0.08)`) when the design-token palette has no alpha variant of the needed colour. The *hue* should still trace to a token where one exists (an amber tint belongs with `--amber`); only the alpha composite may be literal. Opaque colours never qualify.
- **No Tailwind, no CSS modules, no styled-components**.
- Layouts use CSS Grid and Flexbox — no third-party layout libraries.
- Media queries via custom property breakpoints or `@container` queries.

```css
/* src/app.css */
:root {
  --color-bg: #0f0f0f;
  --color-surface: #1a1a1a;
  --color-accent: #6ee7b7;
  --color-text: #e5e5e5;
  --color-muted: #737373;

  --space-xs: 0.25rem;
  --space-sm: 0.5rem;
  --space-md: 1rem;
  --space-lg: 2rem;
  --space-xl: 4rem;

  --radius-sm: 4px;
  --radius-md: 8px;

  --font-body: 'Inter', system-ui, sans-serif;
  --font-mono: 'JetBrains Mono', monospace;
}
```

```svelte
<!-- Component scoped styles -->
<style>
  .card {
    background: var(--color-surface);
    border-radius: var(--radius-md);
    padding: var(--space-md);
  }
</style>
```

---

## Accessibility — the UI is for humans

Every screen here is operated by a **human being** — with a keyboard, sometimes with a screen reader, sometimes with reduced-motion or other preferences set. This is a tool people use for hours; treat keyboard operability and assistive-tech support as correctness requirements, not polish. The small known userbase does **not** lower this bar — these are real people, and at least one may rely on a screen reader or keyboard-only navigation.

Build it right the first time so a manual pass *confirms* accessibility rather than *discovering* its absence.

### Keyboard operability

- **Everything interactive is reachable and operable by keyboard alone.** No mouse-only controls. If you add `onclick`, the element must be a `<button>`/`<a>` (or have a role + `onkeydown`), not a bare `<div>`.
- **Focus must always be visible.** Every focusable control needs a visible focus indicator. Native controls (`<input type="checkbox|radio|search">`, `<select>`) do **not** get an adequate ring for free in this dark theme — `accent-color` colours the control, it does not indicate focus. Add an explicit `:focus-visible` outline (`outline: 2px solid var(--sky); outline-offset: 2px;`).
  - For a small native control where a hugging ring reads poorly (e.g. a radio against its blue `accent-color`), highlight the **enclosing group or label** instead — `.group:focus-within { outline … }` — so the indicator is unmistakable. A radio-group, a labelled checkbox row, and a segmented control are all candidates for group-level focus highlighting.
- **Tab order follows visual/reading order.** Don't reorder with positive `tabindex`. Use `tabindex="-1"` only to remove something from the tab sequence deliberately.

### Dialogs and focus management

- A modal dialog **traps focus** while open (Tab/Shift+Tab cycle within it, computed at interaction time so conditionally-rendered fields participate), moves focus **in** on open, **restores** focus to the opener on close, and dismisses on **Escape** and backdrop click. `Modal.svelte` and `ConfirmDialog.svelte` are the reference implementations — reuse them rather than hand-rolling a dialog.
- Dialogs carry `role="dialog"` (or `alertdialog` for destructive confirmations), `aria-modal="true"`, and `aria-labelledby` pointing at the title; describe the body with `aria-describedby` where there is body text.

### Screen-reader & semantic requirements

- **Use native semantic elements first** — `<button>`, `<a href>`, `<nav>`, `<fieldset>`/`<legend>`, `<label>` wired to its control. ARIA is a fallback for when no native element fits, not a substitute for using the right one.
- **Every control has an accessible name.** A labelled `<label>`, or `aria-label`/`aria-labelledby`. Icon-only buttons **must** have an `aria-label`.
- **Decorative images** (portraits/logos derivable from an id) use `alt=""`; informative images get real alt text.
- **Status and error regions are announced** — `role="status"` / `aria-live="polite"` for transient hints (searching, "no matches"), `role="alert"` for errors. The maps/ACLs pages already follow this; match it.
- **All user-facing strings go through paraglide (`m.*`)** — including `aria-label`s and status text, so assistive tech is localised too. No hardcoded English in markup.

### Motion

- Honour the reduced-motion preference at both layers: the JS tri-state (`data-reduce-motion` override → OS `prefers-reduced-motion`) used by the dialogs, and a `@media (prefers-reduced-motion: reduce)` CSS guard as defence-in-depth. Copy the pattern from `ConfirmDialog.svelte`; don't add unconditional transitions.

### Verifying accessibility

- `svelte-check` flags some a11y issues (missing `alt`, label associations) — treat its a11y warnings as errors, don't suppress them.
- Keyboard behaviour (focus trap, wrap, restore) is **unit-testable** with `@testing-library/svelte` — write those tests; don't defer them to a manual pass.
- The manual keyboard + screen-reader pass is the **final confirmation**, not the first time accessibility is considered. If a manual pass surfaces a missing focus ring or unlabelled control, that's a gap that should have been caught at authoring time — fix it and consider whether the skill needs another rule.

---

## Svelte Flow

Use `@xyflow/svelte` for any graph, canvas, or flow diagram UI.

### Setup

```svelte
<script lang="ts">
  import { SvelteFlow, Controls, Background } from '@xyflow/svelte';
  import '@xyflow/svelte/dist/style.css';
  import type { Node, Edge } from '@xyflow/svelte';

  let nodes = $state<Node[]>([
    { id: '1', position: { x: 0, y: 0 }, data: { label: 'Start' } },
  ]);

  let edges = $state<Edge[]>([]);
</script>

<div class="canvas">
  <SvelteFlow bind:nodes bind:edges fitView>
    <Controls />
    <Background />
  </SvelteFlow>
</div>

<style>
  .canvas {
    width: 100%;
    height: 100%;
  }
</style>
```

### Custom Nodes

Always use custom node components for non-trivial UIs — don't cram markup into `data.label`.

```svelte
<!-- src/lib/components/flow/TaskNode.svelte -->
<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  let { data }: { data: { label: string; status: string } } = $props();
</script>

<div class="task-node" data-status={data.status}>
  <Handle type="target" position={Position.Top} />
  <p>{data.label}</p>
  <Handle type="source" position={Position.Bottom} />
</div>

<style>
  .task-node {
    background: var(--color-surface);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    padding: var(--space-sm) var(--space-md);
  }
  .task-node[data-status='done'] {
    opacity: 0.5;
  }
</style>
```

```svelte
<!-- Register in the parent flow component -->
<script lang="ts">
  import TaskNode from '$lib/components/flow/TaskNode.svelte';
  const nodeTypes = { task: TaskNode };
</script>

<SvelteFlow {nodeTypes} bind:nodes bind:edges>…</SvelteFlow>
```

### Persisting flow state

Save node positions and edge changes via form actions or `+server.ts` PATCH endpoints. Debounce saves on `on:nodechange` / `on:edgechange` events — do not save on every event tick.

```svelte
<script lang="ts">
  import { applyNodeChanges } from '@xyflow/svelte';
  import type { NodeChange } from '@xyflow/svelte';

  let saveTimeout: ReturnType<typeof setTimeout>;

  function handleNodesChange(changes: NodeChange[]) {
    nodes = applyNodeChanges(changes, nodes);
    clearTimeout(saveTimeout);
    saveTimeout = setTimeout(() => persistNodes(nodes), 500);
  }
</script>

<SvelteFlow bind:nodes bind:edges onnodeschanage={handleNodesChange}>…</SvelteFlow>
```

---

## Node Adapter

```ts
// svelte.config.js
import adapter from '@sveltejs/adapter-node';

export default {
  kit: {
    adapter: adapter({
      out: 'build',      // output dir
      precompress: true, // gzip + brotli static assets
    }),
  },
};
```

- Set `PORT`, `HOST`, `ORIGIN` environment variables at runtime — do not hardcode.
- Use `$env/dynamic/private` for runtime secrets (not `$env/static/private` unless the value is truly build-time).
- Session/auth state lives in `locals` — set it in `src/hooks.server.ts`.

```ts
// src/hooks.server.ts
import type { Handle } from '@sveltejs/kit';

export const handle: Handle = async ({ event, resolve }) => {
  const session = await getSession(event.cookies);
  event.locals.user = session?.user ?? null;
  return resolve(event);
};
```

---

## TypeScript

- Strict mode on — `"strict": true` in `tsconfig.json`.
- Use generated `$types` imports in every route file (`PageServerLoad`, `Actions`, `RequestHandler`).
- No `any`. Use `unknown` and narrow properly.
- Shared types in `src/lib/types.ts`; server-only types in `src/lib/server/types.ts`.

---

## Testing Requirements

Frontend tests are not optional. Every non-trivial piece of frontend code gets a test — load functions, form actions, server endpoints, components with logic, rune-state modules, and utility helpers. The only exclusions are trivial glue (pure-presentational components with no logic, one-line helpers, `+page.svelte` files that just render `data` from `load`).

### Tooling

| Test type | Tool | Lives in |
|---|---|---|
| Unit (functions, modules, components in isolation) | **Vitest** + `@testing-library/svelte` | `*.test.ts` co-located next to the file under test |
| End-to-end (real browser, real server) | **Playwright** | `frontend/tests/e2e/` |
| Type checking | `svelte-check` | run in CI; treat warnings as errors |

Both tools are wired up by `npm create svelte@latest` when the test options are selected — keep them. Do not introduce a third test runner.

### Unit Tests — Vitest

Cover every non-trivial unit individually. Mock the boundary (server load mocks `db`, components mock fetched `data`, server endpoints mock services).

**Coverage targets per file type:**

| File | What to test | How to isolate |
|---|---|---|
| `+page.server.ts` load | auth gating, error paths, shape of returned `data` | mock `$lib/server/db` (or whatever it imports) |
| `+page.server.ts` actions | validation branches, redirect vs. `fail` returns, side effects | mock the server module the action calls |
| `+server.ts` endpoints | each method, each status code, payload shape, auth gating | mock the server module the handler calls |
| `+page.svelte` / components | conditional rendering, event handlers, `$derived` correctness, prop variants | render via `@testing-library/svelte`; pass `data`/`form` as props |
| `.svelte.ts` rune state modules | every mutator, every `$derived` | import directly; assert on `$state` values |
| `$lib/utils.ts` / pure helpers | every branch, every edge case | none needed — call the function |
| `hooks.server.ts` | every code path in `handle` (session present/absent, error paths) | mock cookie store and `resolve` |

**Load function example:**

```ts
// src/routes/dashboard/+page.server.test.ts
import { describe, it, expect, vi } from 'vitest';
import { load } from './+page.server';

vi.mock('$lib/server/db', () => ({
  db: { projects: { listByUser: vi.fn().mockResolvedValue([{ id: '1', name: 'p' }]) } },
}));

describe('dashboard load', () => {
  it('returns projects for authenticated user', async () => {
    const result = await load({ locals: { user: { id: 'u1' } } } as any);
    expect(result.projects).toHaveLength(1);
  });

  it('throws 401 when unauthenticated', async () => {
    await expect(load({ locals: { user: null } } as any)).rejects.toMatchObject({ status: 401 });
  });
});
```

**Component example:**

```ts
// src/lib/components/TaskCard.test.ts
import { render, screen } from '@testing-library/svelte';
import TaskCard from './TaskCard.svelte';

it('renders the label', () => {
  render(TaskCard, { props: { task: { id: '1', label: 'ship it', status: 'done' } } });
  expect(screen.getByText('ship it')).toBeInTheDocument();
});

it('applies done state', () => {
  const { container } = render(TaskCard, { props: { task: { id: '1', label: 'x', status: 'done' } } });
  expect(container.querySelector('[data-status="done"]')).not.toBeNull();
});
```

**Rune state example:**

```ts
// src/lib/state/cart.svelte.test.ts
import { describe, it, expect } from 'vitest';
import { cart, addItem } from './cart.svelte';

describe('cart state', () => {
  it('addItem appends to items', () => {
    addItem({ id: '1', qty: 1 });
    expect(cart.items).toHaveLength(1);
  });
});
```

`.svelte.ts` modules need Vitest's Svelte plugin to compile runes. Use `vitest-plugin-svelte` (or the integration that ships with `npm create svelte`) — don't try to test `.svelte.ts` files with the bare `node` environment.

### End-to-End Tests — Playwright

E2E tests exercise the real built app: real server, real browser, real navigation. They are slow; reserve them for **user-visible flows that span pages and persist state**.

- One spec file per flow under `frontend/tests/e2e/`, e.g. `login.spec.ts`, `add-character.spec.ts`.
- Run against a built app (`npm run build && npm run preview`) so the assertions reflect production behaviour, not dev-server quirks.
- Mock the EVE SSO / backend at the network boundary (Playwright's `route.fulfill`) when the test needs to avoid real external calls; otherwise drive against the docker-compose stack.

```ts
// frontend/tests/e2e/login.spec.ts
import { test, expect } from '@playwright/test';

test('unauthenticated visit redirects to /login', async ({ page }) => {
  await page.goto('/');
  await expect(page).toHaveURL(/\/login$/);
  await expect(page.getByAltText(/LOG IN with EVE Online/)).toBeVisible();
});
```

### What NOT to test

- Pure-presentational components with no logic (a `<header class="…">` that just wraps a slot). Snapshot tests for these are noise.
- The framework itself (don't test that `redirect()` actually redirects — trust SvelteKit).
- Style values (don't assert `getComputedStyle(...)` equals a CSS custom property — that's a design system regression test, not a unit test).

### Running tests

- `npm run test` — Vitest in watch mode (dev) or single run (CI).
- `npm run test:e2e` — Playwright headless against the built app.
- Both **must** pass before commit. CI runs `svelte-check` + `vitest run` + `playwright test`.

---

## Checklist Before Committing

- [ ] All reactive state uses `$state`, `$derived`, `$effect` — no `$:` or `export let`
- [ ] Props declared with `$props()` — not `export let`
- [ ] Slots replaced with `{#snippet}` / `{@render}`
- [ ] No `fetch` calls in component scripts for initial data — use `load()`
- [ ] Mutations use form actions (preferred) or `+server.ts`
- [ ] `use:enhance` on all `<form>` elements
- [ ] Styles use CSS custom properties — no hardcoded colours or spacing values
- [ ] Every interactive control is keyboard-operable with a visible `:focus-visible` indicator (native checkbox/radio/select included — group-highlight where a per-control ring reads poorly)
- [ ] Modals reuse `Modal.svelte` / `ConfirmDialog.svelte` (focus trap, focus-in/restore, Escape, backdrop, dialog ARIA) — not hand-rolled
- [ ] Every control has an accessible name; icon-only buttons have `aria-label`; decorative images use `alt=""`
- [ ] Status/error regions use `role="status"` / `role="alert"`; all strings (incl. `aria-label`s) go through paraglide `m.*`
- [ ] Reduced-motion honoured at JS + CSS layers; keyboard behaviour has Vitest coverage
- [ ] Svelte Flow custom nodes are separate components, not inline label markup
- [ ] Flow state persisted via debounced save, not on every change event
- [ ] `$env/dynamic/private` for runtime secrets
- [ ] `locals.user` set in `hooks.server.ts` — not re-fetched in every load function
- [ ] `strict: true` TypeScript, no `any`
- [ ] Vitest unit test for every non-trivial unit — load functions, actions, `+server.ts` handlers, components with logic, `.svelte.ts` state modules, hooks, and helpers
- [ ] Playwright e2e test for every user-visible flow that spans pages or persists state
- [ ] `svelte-check`, `vitest run`, and `playwright test` all pass locally
