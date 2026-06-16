# Design — Align root docs and gate drift

## Decisions

### 1. Implement `ESI_CALLBACK_URL` rather than delete it from the README

Two directions reconcile the README↔code drift: delete the row, or make it real.
We make it real. It costs little — one optional config field and a single derived
value reused at the existing callsites — and it removes a real footgun for proxied
deployments (the documented override silently doing nothing). The variable is an
optional URL with a derived default, so it does not violate the
`project-infrastructure` "no secrets SHALL have default values" rule; that rule is
about *secrets*, and the spec delta states the exemption explicitly.

### 2. Resolve the callback URL once, in config

There are three callsites today that build the callback URL independently
(`handlers/auth.rs`: login ~L65, token exchange ~L138, add-character ~L327), each
formatting `{app_url}/auth/callback`. The spec now requires they cannot diverge.
Implementation: resolve the effective callback URL when building `Config`
(`ESI_CALLBACK_URL` if set, else `format!("{app_url}/auth/callback")`), store it as
a `Config` field, and have all three callsites read that field. This is the
single-source-of-truth the eve-sso-auth delta calls for.

### 3. The "Root-doc upkeep" rule is doc-only, review-enforced — no CI gate

The user chose the doc-only scope. This mirrors the existing "Architecture doc
upkeep" rule for `openspec/AGENTS.md`, which is also review-enforced and has held
up. A CI check (e.g. asserting documented env vars match `config.rs` /
`.env.example`) is more machinery than the precedent it parallels and heavier than
this repo's scale warrants. If drift recurs *despite* the rule, that is the signal
to add a mechanical gate — not before. The rule's job is to make doc reconciliation
a visible, required line in every relevant `tasks.md`, the same way the AGENTS.md
rule does.

### 4. Rewrite, not patch, `frontend/README.md`

It is the stock `sv create` scaffold — there is nothing accurate to preserve. The
rewrite states pnpm-only commands, the three frontend verification commands the
CLAUDE.md rule already mandates, and points at the `sveltekit-node` skill and
`openspec/AGENTS.md` as the authoritative structure sources (rather than
re-documenting the route tree, which would itself drift).

## The rule wording (to add to CLAUDE.md)

A new top-level section, placed adjacent to "Architecture doc upkeep":

> ## Root-doc upkeep
>
> The human-facing root docs (`README.md`, `CONTRIBUTING.md`, `RELEASING.md`,
> `frontend/README.md`, `backend/README.md`) describe how to configure, build,
> deploy, and run the project. A change MUST reconcile the affected root doc **in
> the same change** when it changes a fact one of them states, including:
>
> - configuration / environment variables (names, defaults, required-ness);
> - the deploy or release flow, or published artifacts;
> - local setup steps, prerequisites, or documented commands;
> - route mounts or public URL surface a doc references;
> - the location of tests or other paths a doc names.
>
> When generating a change's `tasks.md`, if the change touches any of the above,
> the tasks MUST include an explicit step to update the affected root doc(s),
> alongside the spec deltas and before the change is marked complete. This is
> review-enforced, like the architecture-doc rule above — keep the docs a current
> description, not a changelog.

## Out of scope

- No CI assertion of doc/config consistency (decision 3).
- No re-documentation of the route tree in `frontend/README.md` (decision 4).
- The accurate docs (`CONTRIBUTING.md`, `RELEASING.md`, `backend/README.md`, root
  `AGENTS.md`, `CLAUDE.md` body) are not rewritten — only `CLAUDE.md` gains the
  new rule section.
