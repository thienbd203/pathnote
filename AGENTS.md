# Prefer existing solutions

Before implementing common infrastructure, search official documentation and existing Rust crates or repositories for a suitable solution. Prefer integrating an existing, maintained implementation over writing a replacement.

Evaluate compatibility with the project's dependency versions, maintenance status, security, and the actual requirements. Do not assume an existing crate is suitable merely because its name matches the task.

When custom code is needed, explain the concrete gap in available solutions and keep the custom portion as small as possible. Clearly distinguish library-provided behavior from application code.

# Architecture standard

Follow `docs/architecture.md` for every new or modified module. Record any change to JSON the frontend sees in its "Thay đổi contract API" table in the same commit, so `../pathnote-web` can follow.

# Docs

Docs live in `docs/` and `deploy/README.md`; update them in the same commit when behavior changes:
- `docs/architecture.md`: code standard and the API contract change log.
- `docs/features.md`: current behavior and API of each feature.
- `docs/roadmap.md`: the only list of open work. Write a short plan under an item when starting it; remove the item when done.
- `deploy/README.md`: VPS setup, coordinated release, backup and rollback.

Do not create new doc or plan files without need; extend these instead. Git history holds older plans.
