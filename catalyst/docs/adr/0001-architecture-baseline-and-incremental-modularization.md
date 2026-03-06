# ADR 0001: Baseline Architecture and Incremental Modularization

- Status: Accepted
- Date: 2026-03-05

## Context

Catalyst currently operates as a functional monolith across the Tauri boundary:

- Backend concerns are concentrated in `src-tauri/src/lib.rs`.
- Frontend orchestration is concentrated in `src/mainPage/mainPage.ts`.
- IPC calls are largely direct `invoke()` usage from page/controller code.

This shape is functional, but it increases change risk, slows onboarding, and limits testability.

## Decision

Refactor the codebase toward a modular structure while keeping the current
Tauri command interface stable during the transition.

The migration will happen incrementally instead of as a large rewrite.

1. Keep existing Tauri command names stable during early migration phases.
2. Add guardrails before behavior changes:
   - command inventory artifact,
   - smoke checks for frontend and backend,
   - strict TypeScript checks for newly introduced modular files.
3. Migrate in phases (frontend IPC boundary, backend extraction, typed errors, service extraction, security hardening).

## Guardrails (Phase 0)

- Command inventory generation script:
  - `npm run inventory:commands`
- Strict checks for new modular TS files:
  - `npm run typecheck:new`
- Smoke checks:
  - `npm run smoke:frontend`
  - `npm run smoke:backend`
  - `npm run smoke`

## Consequences

### Positive

- Safer refactoring through repeatable checks.
- Better visibility into command surface area.
- Stronger type-safety expectations for newly introduced modules.

### Negative / Trade-offs

- Additional scripts and docs to maintain.
- Temporary coexistence of legacy and modularized code during migration.

## Notes

- This ADR authorizes phased implementation and explicitly requires pause/review between phases.
- This ADR does not, by itself, change runtime behavior.