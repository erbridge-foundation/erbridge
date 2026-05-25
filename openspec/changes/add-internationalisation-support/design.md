## Context

The frontend has no i18n infrastructure. All user-facing strings are hardcoded in English across Svelte components. Adding i18n requires choosing a library, establishing a message catalogue convention, wiring locale detection, and (optionally) persisting the user's locale preference via the backend.

## Goals / Non-Goals

**Goals:**
- Introduce a lightweight i18n library compatible with SvelteKit and Svelte 5 runes
- Establish a message catalogue structure and key naming convention
- Replace hardcoded user-facing strings with translation calls
- Detect and apply browser locale on first visit
- Persist user locale preference via the account API

**Non-Goals:**
- Providing translations beyond English at this stage (infrastructure only)
- Right-to-left layout support
- Server-side rendering of translated content beyond what SvelteKit provides by default

## Decisions

<!-- Key technical decisions to be determined during implementation -->

## Risks / Trade-offs

<!-- Risks and trade-offs to be identified during implementation -->
