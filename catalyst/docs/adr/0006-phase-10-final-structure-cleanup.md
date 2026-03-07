---
adr: 0006
title: Phase 10 — Final structure cleanup (Vite inputs and HTML entrypoints)
status: accepted
date: 2026-03-07
---

Context
-------

The repository currently provides two HTML entrypoints: `index.html` (login) and
`main.html` (launcher). `vite.config.ts` is configured with explicit multi-page
inputs for both files. The frontend bootstrap (`src/main.ts`) already queries
session state and redirects to `/main.html` when a valid session exists.

Phase 10 addressed whether to remove duplicate entrypoints and how to simplify
Vite inputs while preserving a clear separation between the login surface and
the logged-in application shell.

Decision
--------

1. Preserve two distinct HTML entrypoints: `index.html` (public login/landing)
   and `main.html` (authenticated launcher). Rationale:
   - Clear separation of concerns simplifies UX expectations for unauthenticated
     and authenticated flows.
   - Smaller, purpose-built entry files improve build-time caching and are
     easier to reason about in an incremental migration.

2. Keep `vite.config.ts` explicit multi-page inputs. The current configuration
   (using `rollupOptions.input` with explicit resolves) is intentional and
   clearer than opaque defaults; we prefer explicitness over compactness for
   multi-entry builds.

3. Ensure the login entrypoint (`index.html`) is skipped client-side when a
   session exists by relying on the frontend session check and redirect in
   `src/main.ts` (or server-side redirects when applicable). This preserves a
   fast path for returning users and keeps the login page logically separate.

4. Offer optional, low-risk simplifications (non-default):
   - Replace `resolve(__dirname, "index.html")`/`resolve(__dirname, "main.html")`
     with relative strings if desired; this is cosmetic and provides no runtime
     benefit in this repo.
   - Consolidate into a single SPA entrypoint (merge login + launcher) is
     rejected for now because it changes deployment model, complicates deep
     linking, and removes the clear unauthenticated landing page.

Consequences
------------

- Positive
  - Maintains explicit, testable entrypoints for both unauthenticated and
    authenticated flows.
  - Avoids a disruptive migration to an SPA while preserving developer
    ergonomics and predictable build outputs.

- Trade-offs
  - Slightly more build output (two HTMLs) and duplicated global CSS/script
    references; accepted as a clarity trade-off.

Implementation Notes
--------------------

- Leave `vite.config.ts` as-is with explicit `input` paths for `index` and
  `main`.
- Ensure `src/main.ts` continues to call `ipcService.getSession()` early in
  the bootstrap and redirects to `/main.html` when a session is present. If
  server-side redirects are later available, add them as an optimization.
- Keep `main.html` referencing `src/mainPage/mainPage.ts` and `index.html`
  referencing `src/main.ts` so the two flows remain decoupled and independently
  testable.

Acceptance Criteria
-------------------

1. `npm run build` produces both `dist/index.html` and `dist/main.html` (or
   equivalent files) and assets referenced by each.
2. Running the app in dev (`npm run tauri dev`) shows `index.html` for new
   sessions and redirects to `main.html` for existing sessions.
3. Frontend tests or smoke runs confirm the redirect path is not causing
   navigation loops and that background event listeners (e.g., `local-scan-*`)
   are registered for both entrypoints where applicable.

Migration / Rollback
--------------------

- Migration is non-destructive: this ADR documents the kept configuration. To
  rollback to a single-SPA approach, revert to a commit that consolidates
  entrypoints and update `vite.config.ts` accordingly.

Follow-ups
----------

1. Add an automated smoke test that runs the built `dist` site in a small
   static server and verifies that `index.html` redirects when a persisted
   session token exists.
2. Optional cosmetic cleanup: replace `resolve(__dirname, ...)` uses in
   `vite.config.ts` with relative paths if desired for terseness.

Files
-----

- Vite inputs: `vite.config.ts`
- Login entrypoint: `index.html`
- Launcher entrypoint: `main.html`
- Login bootstrap: `src/main.ts`
