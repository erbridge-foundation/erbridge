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

## Checklist Before Committing

- [ ] All reactive state uses `$state`, `$derived`, `$effect` — no `$:` or `export let`
- [ ] Props declared with `$props()` — not `export let`
- [ ] Slots replaced with `{#snippet}` / `{@render}`
- [ ] No `fetch` calls in component scripts for initial data — use `load()`
- [ ] Mutations use form actions (preferred) or `+server.ts`
- [ ] `use:enhance` on all `<form>` elements
- [ ] Styles use CSS custom properties — no hardcoded colours or spacing values
- [ ] Svelte Flow custom nodes are separate components, not inline label markup
- [ ] Flow state persisted via debounced save, not on every change event
- [ ] `$env/dynamic/private` for runtime secrets
- [ ] `locals.user` set in `hooks.server.ts` — not re-fetched in every load function
- [ ] `strict: true` TypeScript, no `any`
