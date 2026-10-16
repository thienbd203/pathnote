# Prefer existing solutions

Before implementing common infrastructure, search official documentation and existing Rust crates or repositories for a suitable solution. Prefer integrating an existing, maintained implementation over writing a replacement.

Evaluate compatibility with the project's dependency versions, maintenance status, security, and the actual requirements. Do not assume an existing crate is suitable merely because its name matches the task.

When custom code is needed, explain the concrete gap in available solutions and keep the custom portion as small as possible. Clearly distinguish library-provided behavior from application code.

# Architecture standard

Follow `plans/10-architecture.md` (Part A) for every new or modified module. When working on the standardization effort, take tasks from Part B in order, one commit per task, and record any API contract change in table B.6 so the frontend (`../pathnote-web/docs/architecture.md`) can follow.
