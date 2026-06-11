## Context

The ACL member picker (`MemberPicker.svelte`) searches all three ESI categories via the `?/search` form action, which calls `searchEntities(...)` → `GET /api/v1/entities/search`. The backend endpoint already accepts an optional `categories` query parameter (comma-separated subset of `character,corporation,alliance`; omitted/empty/unknown → all three), and the `searchEntities(fetch, backendUrl, q, cookie, categories?)` client function already forwards it. Neither was exposed in the UI: the picker never sent a category filter.

This change exposes that existing capability. It is frontend-only and was shipped on `develop` in commit `1b95d69`; this artifact documents the decisions retroactively.

## Goals / Non-Goals

**Goals:**

- Let the user scope the member search to a single category or all, from the picker.
- Make the scope submit with the existing search form (no extra request, no client-side fetch).
- Reuse the backend `categories` param and `searchEntities` client unchanged.

**Non-Goals:**

- No backend change, no API-contract change, no new dependency.
- No multi-select of categories (the backend supports it, but the UX is a single radio choice: one category or "any").
- No persistence of the chosen scope across searches/sessions.

## Decisions

- **Radio group, not a `<select>` or checkboxes.** Four mutually-exclusive options (`character` / `corporation` / `alliance` / `any`) read best as radios, and `any` makes "search everything" an explicit first-class choice rather than an empty state. Default is `any`, preserving the prior behavior so existing flows and e2e tests are unaffected.
- **Scope lives inside the search `<form>`.** The radios submit as a `scope` field alongside `q` on the same POST, so there is no separate request and no client-side `fetch`; the server action owns the ESI call as before.
- **`any` maps to omitting `categories`, not sending all three.** The server action sends `categories` only for a single concrete category; `any` (or any unrecognized value) leaves `categories` undefined so the backend applies its own all-three default. This keeps the frontend decoupled from the backend's category token list — the canonical "all" set lives in one place (the backend).
- **Mapping lives in the `search` action, not the component.** The component emits a raw `scope` form value; `acls/[id]/+page.server.ts` translates it to the `categories` argument. This keeps the component free of API-shape knowledge, consistent with how the picker already submits raw fields the action interprets.

## Risks / Trade-offs

- [User expects multi-category selection] → Out of scope by design; the single-choice radio + `any` covers the common "I know what I'm adding" case. The backend already supports comma-separated subsets if a future change wants multi-select.
- [Scope not persisted, resets to `any` each render] → Acceptable; the picker is a transient add affordance and the default is the safe superset.
