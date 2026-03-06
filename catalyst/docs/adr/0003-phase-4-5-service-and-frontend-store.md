# ADR 0003 — Phase 4 & Phase 5: Domain Service Extraction and Frontend Store Extraction

- Status: Accepted
- Date: 2026-03-06

## Context

Phase 0 through Phase 3 (see ADR 0001 and ADR 0002) established the migration strategy and moved Tauri command handlers into `src-tauri/src/interface/tauri/commands/*` while preserving public IPC surface area. The repository's PLAN (see `PLAN.md`) identifies Phase 4 and Phase 5 as the next safe, incremental steps:

- Phase 4: Move domain logic (collections, library/game settings, Steam integration, and game actions) into a dedicated `application/services` layer so command adapters remain thin.
- Phase 5: Move frontend mutable module-level state out of the `mainPage.ts` orchestrator and into a small feature store (`libraryStore.ts`) with selectors.

Both phases are small, well-scoped refactors intended to improve separation-of-concerns, testability, and maintainability while preserving runtime behavior during migration.

## Decision

We will implement a single ADR covering both phases and authorize the following decisions:

1. Phase 4: Extract domain logic from command modules into `src-tauri/src/application/services/*`. Command modules under `src-tauri/src/interface/tauri/commands/*` become thin adapters that validate inputs and delegate to service functions. Services are the canonical place for domain behavior and unit tests.

2. Phase 5: Create a small frontend feature store at `src/mainPage/libraryStore.ts` that centralizes the mutable state currently defined in `src/mainPage/mainPage.ts`. Expose a minimal selector/utility surface to the page controller so rendering code remains unchanged at call sites.

3. Perform both refactors incrementally per-command and per-feature group. Each small change must keep the repository buildable and passing the guardrail checks defined in ADR 0001 (`cargo check`, `npm run build`) before proceeding to the next group.

## Rationale

- Moving domain logic into services creates a clear seam for unit testing and later dependency injection (e.g., mocking DB or Steam adapters).
- Thin command adapters minimize the protocol surface and reduce test surface area for IPC wiring.
- Centralizing frontend state reduces accidental shared-mutable-state bugs, makes selectors testable, and prepares the UI for future stricter store APIs (actions, immutability) without a large one-shot rewrite.

## Consequences

Positive:

- Improved testability (unit tests can target services and selectors).
- Clearer module boundaries: `interface` = protocol adapters, `application/services` = domain logic, frontend `features/library` = store/selectors.
- Easier, safer refactors and smaller code review changes.

Trade-offs / Risks:

- Short-term duplication may exist while moving logic; keep changes small and reversible.
- Hot-reload/HMR timing can surface assignment-to-import warnings during the migration on the frontend; those are non-fatal but should be addressed in follow-ups.

## Implementation

Phase 4 (backend service extraction) implementation guidance:

1. For each command group (recommended order: `collections`, `library`, `game_settings`, `steam`, `game_actions`):
   - Add `src-tauri/src/application/services/{group}_service.rs` and implement service functions mirroring the command logic.
   - Update `src-tauri/src/interface/tauri/commands/{group}.rs` so each `#[tauri::command]` function delegates to the service function and maps service errors to the adapter `ErrorPayload`.
   - Keep signatures and observable behavior identical during the initial move.
   - Run `cargo check` and existing smoke checks after each group.

2. Add unit tests for service functions where practical using temporary DB fixtures (e.g., `tempfile` for SQLite path) and small fixture helpers to create users/sessions.

3. Gradually remove duplicated logic from command modules once tests and builds validate the service implementations.

Phase 5 (frontend store extraction) implementation guidance:

1. Create `src/mainPage/libraryStore.ts` exporting a `store` object that contains the previously module-level mutable state: `allGames`, `gameById`, `isLoadingLibrary`, `allCollections`, timers, download-tracking maps, and the active view mode enum.

2. Export a small set of selectors and helpers (for example: `getAllGames()`, `findGameById(id)`, `isLibraryViewMode(mode)`) used by `mainPage.ts` and the UI components.

3. Replace direct module-level globals in `src/mainPage/mainPage.ts` with reads/writes to the `store` object, keeping rendering calls unchanged.

4. Build (`npm run build`) and verify HMR behavior in dev; fix any transient assignment-to-import warnings by ensuring consumers mutate `store.*` rather than reassign imported bindings.

## Migration Checklist

- [ ] Create service modules for `collections`, `library`, `game_settings`, `steam`, `game_actions`.
- [ ] Update command modules to delegate to services and run `cargo check` after each group.
- [ ] Add unit tests for service functions using temporary DBs.
- [ ] Create `src/mainPage/libraryStore.ts` and update `src/mainPage/mainPage.ts` to use the store.
- [ ] Run `npm run build` and `npm run smoke` after frontend changes.
- [ ] Remove duplicated logic from command modules after services are validated.

## Acceptance Criteria

1. `cargo check --manifest-path src-tauri/Cargo.toml` passes after each backend group extraction.
2. `npm run build` completes successfully after frontend store extraction.
3. Command adapters still expose the same IPC surface (no renames) and return consistent results for the same inputs.
4. New unit tests for services exist and pass locally.

## Rollback Plan

Because changes are incremental and kept small, rollback is a matter of reverting the commits that introduced a given group's service extraction. Keep single-group changes in individual commits to simplify reverts.

## Testing and Validation

- Backend: add unit tests for service functions and run `cargo test` in `src-tauri` during the extraction.
- Frontend: run `npm run build` and smoke-run the pages in dev (Vite) to validate runtime behavior; ensure HMR warnings are addressed where possible.

## Alternatives Considered

- One-shot rewrite of backend and frontend: rejected for risk and reviewability.
- Leaving code as-is: rejected because it blocks testability and future feature work.

## References

- ADR 0001: Baseline Architecture and Incremental Modularization — [docs/adr/0001-architecture-baseline-and-incremental-modularization.md](docs/adr/0001-architecture-baseline-and-incremental-modularization.md)
- ADR 0002: Phase 2 & Phase 3 modularization — [docs/adr/0002-phase-2-3-modularization.md](docs/adr/0002-phase-2-3-modularization.md)
- PLAN: [PLAN.md](PLAN.md)
