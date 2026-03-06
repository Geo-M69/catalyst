# ADR 0002 — Phase 2 & Phase 3: Incremental Modularization of Tauri Commands and Service Extraction

Status: Accepted

Date: 2026-03-05

## Context

The repository previously contained a monolithic Tauri backend with command handlers implemented inline in `src-tauri/src/lib.rs` and the frontend calling Tauri via untyped `invoke()` calls. Phase 1 introduced a typed IPC boundary for the frontend (`src/shared/ipc/contracts.ts` and `src/shared/ipc/client.ts`) and replaced direct `invoke()` usage in the main UI code.

Phase 2 performed a mechanical extraction of all `#[tauri::command]` functions from `src-tauri/src/lib.rs` into namespaced command modules under `src-tauri/src/interface/tauri/commands/*`. The extraction preserved function signatures, SQL statements, and behavior. `lib.rs` was updated to register command handlers using fully-qualified module paths.

Phase 3 proposes the next incremental step: move the internal business logic and side-effects out of the command modules and into a thin service layer under `src-tauri/src/application/services/*`. Commands will become small adapters that validate inputs and call service APIs. The change must preserve behavior and tests.

Related files and locations:

- Frontend IPC: [src/shared/ipc/contracts.ts](src/shared/ipc/contracts.ts), [src/shared/ipc/client.ts](src/shared/ipc/client.ts)
- Tauri command modules: [src-tauri/src/interface/tauri/commands](src-tauri/src/interface/tauri/commands)
- Top-level Tauri entry: [src-tauri/src/lib.rs](src-tauri/src/lib.rs)
- Service layer scaffold: [src-tauri/src/application/services](src-tauri/src/application/services)

## Decision

1. Document Phase 2 as an accepted, non-behavioral refactor: commands were moved into `interface::tauri::commands` modules with `pub(crate)` visibility and still annotated with `#[tauri::command]`.
2. Proceed with Phase 3: incrementally extract command internals into service modules under `src-tauri/src/application/services`. Each command file will be converted to a thin adapter that calls the corresponding service function. The service function will contain the original implementation unchanged during the initial move (copy-and-reuse), then be the canonical place for subsequent refactors and unit tests.
3. Preserve public IPC surface and SQL semantics during every sub-step; validate with `cargo check`, `npm run build`, and integration smoke tests after each extracted command group.

## Rationale

- Incremental extraction minimizes risk: moving code in small groups keeps the repo buildable and easy to review.
- A service layer provides a natural seam for unit testing and code reuse, separating protocol (Tauri command) from domain logic.
- Preserving command signatures and SQL during Phase 3 ensures no behavior regression and makes the migration reversible.

## Consequences

- Positive:
  - Improved module boundaries and testability.
  - Clear separation of concerns: `interface` for protocol adapters, `application/services` for domain logic.
  - Easier future refactors (e.g., dependency injection, async orchestration, platform-specific implementations).

- Risks / Mitigations:
  - Risk: accidental behavioral change while moving code. Mitigation: copy implementation to service, keep command calling original logic (not replacing until validated), run `cargo check` and the frontend build after each change.
  - Risk: duplicated code during initial move. Mitigation: prefer move-over-copy when safe; otherwise keep both and remove the original after tests pass.

## Implementation Plan (Phase 3)

Migration proceeds by command groups (`collections`, `auth`, `library`, etc.).

For each group:

1. Create a service module in `application/services`.
2. Move command logic into a service function.
3. Update the command to call the service.
4. Run `cargo check` and `npm run build`.
5. Add unit tests where practical.

## Migration Checklist

- [ ] Pick the first group to extract (recommended: `collections` or `auth`).
- [ ] Implement service module scaffold and copy implementation.
- [ ] Update command functions to call the service.
- [ ] Run `cargo check` and `npm run build`.
- [ ] Add unit tests for service functions.
- [ ] Remove duplicated original logic from command files.
- [ ] Repeat for next groups.

## Rollback Plan

Because Phase 3 is incremental and each step preserves the command signatures and behavior, rollback is as simple as reverting the commits for the current group. Keep commits small and atomic to make reversion straightforward.

## Alternatives Considered

- Doing a large, one-shot rewrite of all commands into services: rejected because it increases risk and makes review difficult.
- Leaving the code as-is: rejected because it hampers testability and future evolution.

## References

- Baseline ADR: [docs/adr/0001-architecture-baseline-and-incremental-modularization.md](docs/adr/0001-architecture-baseline-and-incremental-modularization.md)