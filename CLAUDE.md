# E-R Bridge — Project Rules for Claude

## Skill authority

Skills are the authoritative source for architecture, structure, and convention in this project. When a skill is in conflict with any other source (tasks, specs, prior code, or your own judgment), **the skill wins**.

Specific cases:

- **`rust-rest-api` skill defines the authoritative module layout for `backend/`.** Tasks and specs may name illustrative file paths; if any path conflicts with the skill's layout, follow the skill and correct the task path — do not follow the task path and do not update the skill to match.
- **`sveltekit-node` skill defines the authoritative structure for `frontend/`.** Same rule applies.

If you believe a skill rule is wrong or needs updating, **stop and raise it with the user** rather than silently working around it.
