# Message catalogue

User-facing strings live here, one JSON file per locale (`en.json` is the base
locale). They are compiled by Paraglide into tree-shakeable functions under
`src/lib/paraglide/messages` (gitignored, regenerated on build). Import them as:

```ts
import { m } from '$lib/paraglide/messages';
m.prefs_action_apply(); // "Apply"
```

## Key naming convention

Keys are **flat, snake_case, feature-scoped**: `<feature>_<thing>[_<variant>]`.

The inlang message-format plugin keys are a flat namespace (no nested objects),
so the structure lives in the key itself, not in JSON nesting.

| Segment      | Rule                                                              | Examples                                  |
| ------------ | ----------------------------------------------------------------- | ----------------------------------------- |
| `<feature>`  | The page / domain the string belongs to.                          | `nav`, `prefs`, `characters`, `login`     |
| `<thing>`    | What the string is.                                               | `heading`, `intro`, `text_size_label`     |
| `[_<variant>]` | Optional qualifier for related strings (label/description, options). | `_label`, `_description`, `_option_on` |

Examples:

- `nav_characters` — the Characters nav item.
- `prefs_heading` — the preferences page heading.
- `prefs_text_size_label` / `prefs_text_size_description` — a control's label and help text.
- `prefs_action_apply` — the Apply button.

## Adding a string

1. Add the key to `en.json` (and every other locale file once more ship).
2. Use `m.<key>()` in the component — never a hardcoded literal.
3. Messages with placeholders use `{name}` and are called `m.key({ name })`.

Run `pnpm build` (or `pnpm dev`) to recompile; `svelte-check` will fail on a
missing or misspelled key, since the generated functions are typed.
