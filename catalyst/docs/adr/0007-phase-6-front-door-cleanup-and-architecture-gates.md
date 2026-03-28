---
adr: 0007
title: Backend Architecture Redesign and Front-Door Enforcement
status: accepted
date: 2026-03-28
---

Context
-------

Catalyst started as a functional backend monolith centered in
`src-tauri/src/lib.rs`. That shape was effective for early delivery, but over
time it created growing risks:

- large blast radius for routine changes,
- unclear ownership between command wiring and business logic,
- weak boundaries between domain logic and system concerns (DB, HTTP, filesystem),
- architectural drift during incremental refactors.

By this phase, service extraction had begun, but the repository still needed a
clear target design and mechanical safeguards to prevent regressions.

Decision
--------

Catalyst adopts a layered backend design with an explicit front door:

1. Introduce a layered module structure:
   - `domain`: core concepts and business rules (`game`, `collection`, `session`, `settings`, etc.),
   - `application`: orchestration/use-case services, app bootstrap, and port traits,
   - `infrastructure`: adapter implementations for SQLite, Steam APIs, local Steam/launcher effects, cache,
   - `interface`: Tauri-facing command boundary.

2. Keep `src-tauri/src/lib.rs` as a strict crate front door only:
   - module declarations,
   - shared app-state export,
   - inclusion of runtime implementation via `include!("lib_runtime_impl.rs");`.

3. Move operational runtime implementation out of the front door into
   `src-tauri/src/lib_runtime_impl.rs` so composition and execution logic are
   separate concerns.

4. Add architecture gates to enforce the design continuously:
   - `scripts/check-lib-front-door.mjs` ensures `lib.rs` remains front-door-only,
   - `scripts/check-crate-glob-imports.mjs` forbids wildcard imports in application services outside tests.

5. Wire these checks into repeatable npm commands:
   - `guard:lib-front-door`,
   - `guard:crate-glob-imports`,
   - `guard:architecture`,
   - `phase0:guardrails` includes architecture checks plus command/ smoke checks.

Why this was done
-----------------

This redesign was done to make backend evolution safer and cheaper over time:

- preserve velocity while reducing accidental coupling,
- make architecture intent visible in directory/module boundaries,
- isolate system integration code from business logic,
- turn architectural expectations into executable checks rather than tribal knowledge.

Consequences
------------

- Positive
  - Clearer separation of concerns and improved maintainability.
  - Reduced chance of reintroducing monolithic patterns at the crate root.
  - Faster reviews: boundary violations are caught by scripts before merge.
  - Better onboarding: the directory structure communicates responsibility.

- Trade-offs
  - `lib_runtime_impl.rs` is still large and remains an intermediate step, not
    the end-state of decomposition.
  - Guard scripts become part of project maintenance and must evolve with the
    architecture.
  - The stricter boundaries can add short-term refactor overhead.

Implementation Notes
--------------------

- `src-tauri/src/lib.rs` is intentionally minimal and guarded.
- Layer modules now exist in `src-tauri/src/domain`, `application`,
  `infrastructure`, and `interface`.
- Architecture gates are executed through npm scripts in `package.json`.

Acceptance Criteria
-------------------

1. `src-tauri/src/lib.rs` contains only front-door composition lines and passes
   `npm run guard:lib-front-door`.
2. Service modules pass `npm run guard:crate-glob-imports`.
3. `npm run guard:architecture` and `npm run phase0:guardrails` pass locally.
4. Backend still compiles and runs with unchanged Tauri command behavior.

Follow-ups
----------

1. Continue shrinking `lib_runtime_impl.rs` by moving use-case logic into
   `application/services` and adapter-specific logic into `infrastructure`.
2. Expand port/adapter usage so service modules depend on explicit contracts
   instead of shared globals.
3. Add additional architecture checks as boundaries stabilize (for example,
   preventing infrastructure imports directly from interface layer where not intended).
