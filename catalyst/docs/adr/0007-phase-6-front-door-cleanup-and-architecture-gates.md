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

6. Prioritize decomposition of transitional monolith adapters in this order:
   - Steam auth/runtime callback orchestration,
   - Library metadata and store/community fetch logic,
   - Game settings and Steam localconfig/cloudstorage application logic,
   - Collections import/merge/persistence logic,
   - Launcher operations (URI launch, shortcuts, file-manager actions).
   The target end-state is bounded infrastructure modules for each area with
   `lib_runtime_impl.rs` and `infrastructure/library_port.rs` kept as thin
   adapter/delegation surfaces only.

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

1. Continue shrinking `lib_runtime_impl.rs` and `infrastructure/library_port.rs`
   with this extraction order:
   - Steam auth,
   - metadata,
   - settings,
   - collections,
   - launcher ops.
2. For each extracted slice, keep adapter files delegation-only and move
   operational logic into bounded infrastructure modules.
3. Expand port/adapter usage so service modules depend on explicit contracts
   instead of shared globals.
4. Add additional architecture checks as boundaries stabilize (for example,
   preventing infrastructure imports directly from interface layer where not intended).

Implementation Status Update (2026-03-28)
-----------------------------------------

This ADR has now been implemented beyond the initial acceptance criteria.

Completed changes:

1. Enforced front-door and architecture gates:
   - `src-tauri/src/lib.rs` remains front-door-only and is guarded.
   - Architecture guards now include:
     - `check-lib-front-door.mjs`
     - `check-crate-glob-imports.mjs`
     - `check-service-runtime-imports.mjs`
     - `check-shared-boundaries.mjs`
     - `check-max-file-lines.mjs`
   - `phase0:guardrails` passes with inventory check, architecture gates, and smoke checks.

2. Strengthened command boundary and async behavior:
   - Added shared blocking command helper in
     `src-tauri/src/interface/tauri/commands/blocking.rs`.
   - Migrated blocking-heavy Tauri commands to `run_blocking(...)` wrappers
     for safer UI responsiveness.
   - Command inventory generation now verifies annotated commands against the
     generated handler registration and docs inventory.

3. Reduced service monolith risk and extracted focused modules:
   - `library_downloads_service.rs`
   - `library_store_metadata_service.rs`
   - `library_review_service.rs`
   - shared response types moved to `library_types.rs`

4. Completed service-to-port migration for non-legacy services:
   - Added application ports and infrastructure adapters for:
     - auth
     - steam
     - game actions
     - game settings
     - library
   - `LEGACY_EXCEPTIONS` in `check-service-runtime-imports.mjs` is now empty.
   - Application service modules are now façade/orchestration layers over
     explicit port contracts.

5. Added frontend/shared boundary and modularization guardrails:
   - Shared boundary gate prevents `src/shared` from importing `src/mainPage`.
   - Introduced shared IPC models/contracts and extracted frontend helpers
     (`libraryUiHelpers.ts`, `detailsDropdownMetadata.ts`) to reduce coupling.

6. Continued `lib_runtime_impl.rs` decomposition by extracting focused runtime helpers:
   - Added:
     - `src-tauri/src/infrastructure/runtime_auth.rs`
     - `src-tauri/src/infrastructure/runtime_session_state.rs`
     - `src-tauri/src/infrastructure/runtime_database.rs`
     - `src-tauri/src/infrastructure/runtime_http.rs`
   - `lib_runtime_impl.rs` now delegates auth/session/database/http helper logic
     to these modules while preserving crate-level helper entry points used by
     existing services/adapters.
   - Removed duplicated inlined helper implementations from
     `lib_runtime_impl.rs` and reduced the file from 7260 lines to 6843 lines
     in this transition step.

7. Extracted Steam callback/OpenID runtime helpers:
   - Added `src-tauri/src/infrastructure/runtime_steam_callback.rs` for:
     - callback host resolution/fallback,
     - callback HTTP request/response handling,
     - Steam authorization URL construction,
     - OpenID verification request,
     - Steam ID extraction from callback params.
   - `complete_steam_auth_flow(...)` now delegates to this module instead of
     carrying those helpers inline.
   - Further reduced `lib_runtime_impl.rs` from 6843 lines to 6680 lines.

8. Extracted Steam auth orchestration into infrastructure runtime auth:
   - Moved `complete_steam_auth_flow(...)` orchestration and Steam user
     resolution logic into `src-tauri/src/infrastructure/runtime_auth.rs`.
   - `src-tauri/src/lib_runtime_impl.rs` now keeps `complete_steam_auth_flow`
     as a thin delegation wrapper only.
   - Removed now-obsolete auth wrappers from `lib_runtime_impl.rs`.
   - Reduced `lib_runtime_impl.rs` from 6680 lines to 6571 lines in this step.

9. Started metadata decomposition in infrastructure with a bounded DLC module:
   - Added `src-tauri/src/infrastructure/library_steam_dlc.rs` to own Steam
     DLC metadata fetch, name resolution, dedupe, and response assembly logic.
   - `src-tauri/src/infrastructure/library_port.rs` now delegates
     `get_game_dlc(...)` to this module as a thin adapter call.
   - Reduced `library_port.rs` from 2861 lines to 2378 lines in this step.

10. Continued metadata decomposition with achievements/trading-cards module:
   - Added `src-tauri/src/infrastructure/library_steam_progress.rs` to own
     Steam achievements and trading-cards metadata orchestration/caching logic.
   - `src-tauri/src/infrastructure/library_port.rs` now delegates
     `get_game_achievements(...)` and `get_game_trading_cards(...)` as thin
     adapter calls.
   - Reduced `library_port.rs` from 2378 lines to 1647 lines in this step.

11. Continued metadata decomposition with social/timeline module:
   - Added `src-tauri/src/infrastructure/library_steam_social.rs` to own Steam
     friends-activity and activity-timeline orchestration/parsing logic.
   - `src-tauri/src/infrastructure/library_port.rs` now delegates
     `get_game_friends_activity(...)` and `get_game_activity_timeline(...)` as
     thin adapter calls.
   - Reduced `library_port.rs` from 1647 lines to 277 lines in this step.

12. Started settings boundary decomposition into infrastructure runtime settings:
   - Added `src-tauri/src/infrastructure/runtime_steam_settings.rs` to own
     Steam settings/localconfig/sharedconfig/cloudstorage/privacy/properties
     logic and persistence helpers.
   - `src-tauri/src/lib_runtime_impl.rs` now keeps settings entry points as
     thin delegation wrappers to this module:
     - `normalize_game_properties_settings_payload(...)`
     - `load/save_game_properties_settings(...)`
     - `resolve_steam_compatibility_tools(...)`
     - `clear_steam_game_overlay_data(...)`
     - `apply_steam_game_privacy_settings(...)`
     - `apply_steam_game_properties_settings(...)`
     - `load/save_game_privacy_settings(...)`
   - Reduced `lib_runtime_impl.rs` from 6571 lines to 5205 lines in this step.

13. Started collections boundary decomposition into infrastructure runtime collections:
   - Added `src-tauri/src/infrastructure/runtime_collections.rs` to own Steam
     collections parsing/merge/import/persistence helpers:
     - `parse_steam_collections_from_vdf(...)`
     - `merge_collections_by_app_id(...)`
     - `import_steam_collections_for_user(...)`
   - `src-tauri/src/infrastructure/steam_port.rs` now imports and delegates to
     `runtime_collections` directly for Steam collections orchestration.
   - Removed inlined collection import/merge/persistence helper
     implementations from `src-tauri/src/lib_runtime_impl.rs`.
   - Reduced `lib_runtime_impl.rs` from 5205 lines to 4985 lines in this step.

14. Started launcher-operations boundary decomposition into infrastructure runtime launcher ops:
   - Added `src-tauri/src/infrastructure/runtime_launcher_ops.rs` to own:
     - provider URI launch orchestration,
     - Steam URI dispatch fallback behavior,
     - desktop shortcut creation,
     - file-manager open operations.
   - `src-tauri/src/infrastructure/launcher_ops.rs` now remains a thin adapter
     delegating directly to `runtime_launcher_ops`.
   - Removed inlined launcher operation implementations from
     `src-tauri/src/lib_runtime_impl.rs`.
   - Reduced `lib_runtime_impl.rs` from 4985 lines to 4638 lines in this step.

15. Extracted shared VDF parsing/manipulation infrastructure:
   - Added `src-tauri/src/infrastructure/runtime_vdf.rs` to own:
     - VDF tokenization/parsing,
     - object traversal helpers,
     - text entry mutation helpers,
     - VDF serialization helpers.
   - Rewired VDF consumers to explicit bounded imports:
     - `src-tauri/src/infrastructure/runtime_steam_settings.rs`
     - `src-tauri/src/infrastructure/runtime_collections.rs`
   - Removed the inlined VDF helper cluster from
     `src-tauri/src/lib_runtime_impl.rs`.
   - Reduced `lib_runtime_impl.rs` from 4638 lines to 4304 lines in this step.

Current transitional state:

- The architecture gates and service boundaries are now enforced.
- The remaining transition work is to keep shrinking
  `src-tauri/src/lib_runtime_impl.rs` by moving runtime helper functions into
  dedicated modules/adapters while preserving behavior.

Priority Update (2026-04-02)
----------------------------

To keep Phase 6 focused, decomposition work is now explicitly prioritized
around bounded infrastructure modules and thin adapters:

1. Steam auth boundary first:
   - Move remaining auth/runtime orchestration from `lib_runtime_impl.rs` into
     infrastructure auth modules, keeping callback helpers and auth state logic
     outside the crate front-door runtime file.
2. Metadata boundary second:
   - Split `infrastructure/library_port.rs` metadata/store/community operations
     into dedicated metadata modules and keep `library_port.rs` as orchestration
     glue over ports and adapters.
3. Settings boundary third:
   - Extract Steam settings/localconfig/sharedconfig/cloudstorage application
     logic into dedicated settings modules with explicit entry points.
4. Collections boundary fourth:
   - Extract Steam collections parsing/import/persistence helpers into focused
     collections modules.
5. Launcher ops boundary fifth:
   - Keep URI/process/shortcut/file-manager operations in dedicated launcher
     infrastructure modules with minimal adapter wrappers.

Guardrail expectation:

- Architecture guardrails should prevent growth of
  `src-tauri/src/lib_runtime_impl.rs` and
  `src-tauri/src/infrastructure/library_port.rs` while extraction proceeds.
