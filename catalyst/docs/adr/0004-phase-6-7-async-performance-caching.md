# ADR 0004 — Phase 6 & Phase 7: Async performance hardening, background workers, and caching

- Status: Accepted
- Date: 2026-03-06

## Context

Phases 6 and 7 of the migration plan focus on reducing UI blocking behavior and lowering runtime cost of frequent Steam and local filesystem operations. The codebase currently contains several synchronous, blocking paths invoked from the Tauri command surface (e.g., blocking `reqwest` store calls, heavy local Steam filesystem scans), which lead to latency spikes and UI jank. Phase 6 consolidated architecture and modularization changes; Phase 7 targets operational hardening:

- Move long-running filesystem and network work off the UI command thread into worker paths.
- Add short-lived in-memory and DB-backed caches to reduce repeated expensive network and filesystem calls.
- Batch high-volume Steam Store requests where appropriate and favor bulk endpoints over per-item calls.
- Provide event-driven notifications for background work completion to decouple command latency from UX responsiveness.

Relevant work already performed in the codebase during development includes:

- An in-memory TTL cache for local Steam install detection.
- A non-blocking Tauri command (`start_local_steam_scan`) that spawns a background thread and emits `local-scan-complete` / `local-scan-error` events.
- A DB-backed `steam_app_details` cache with helpers `find_cached_steam_app_details` and `cache_steam_app_details`.
- Integration of the `steam_app_details` cache into several single-app callers (languages, install-size estimate, linux-platform support).

This ADR codifies the Phase 6/7 decisions, implementation patterns, and acceptance criteria so subsequent work can remain consistent and auditable.

## Decision

1. Long-running local filesystem scans and blocking network calls MUST be moved off the synchronous Tauri command thread. Commands will remain thin adapters that either:
   - Immediately spawn a background worker (OS thread, thread-pool), return success to the caller, and emit completion events to the frontend; or
   - Use an async service pattern inside the backend with explicit non-blocking primitives (where available) and provide progress/completion events.

2. Two-tier caching strategy will be adopted:
   - Short-lived in-memory TTL cache(s) for high-frequency, cheap-to-recompute results (example: `local_installed_app_ids`, 5 minute TTL).
   - DB-backed persistent JSON caches for heavier remote payloads (example: `steam_app_details`, default TTL ~1 week) stored in the application's SQLite database with `fetched_at` metadata.

3. Cache semantics:
   - Reads are guarded by TTL checks (`fetched_at >= now - ttl`), and stale entries result in background refresh or synchronous fallback fetch depending on caller semantics.
   - Cache writes are best-effort: failures to write the cache do not cause the primary caller to fail.
   - Bulk/batched endpoints should be preferred for library-scale operations; DB cache may store either the batched response or per-app blobs depending on downstream access patterns.

4. Event-driven notification model:
   - Background workers emit typed events on completion or error (e.g., `local-scan-complete: number[]`, `local-scan-error: string`) via the Tauri `AppHandle::emit` surface.
   - The frontend registers listeners for those events and updates UI state incrementally.

5. Testing and observability:
   - Unit tests for caching helpers and worker helpers are required.
   - Integration tests that run the worker helper in-process and capture emitted events (via injected closures or testing channels) are required to validate behavior deterministically.

6. Backwards compatibility and migration rules:
   - Public Tauri command names and signatures must remain stable during migration; new command variants may be added, but existing consumers must not be broken.
   - Where a synchronous fallback remains (for callers that must return data immediately), prefer returning conservative results and kicking off an asynchronous refresh rather than blocking on heavy calls.

## Rationale

- Moving blocking work out of command handlers avoids UI thread starvation and improves perceived responsiveness.
- Two-tier caching balances freshness with performance: in-memory caches avoid repeated filesystem re-scans while DB caches reduce repeated small network requests to the Steam Store for single-app callers.
- Best-effort cache writes preserve caller success semantics and avoid turning transient cache problems into user-visible failures.
- Event-based completion aligns with progressive UI updates and allows the frontend to display intermediate loading state while data refreshes complete.

## Implementation Guidance

The following patterns should be followed when implementing Phase 6/7 work:

1. Background worker helper (Rust):
   - Provide a small public helper that runs the blocking task and accepts an emitter closure `FnOnce(Result<SuccessPayload, String>) + Send + 'static` so tests can capture results without launching a full Tauri runtime.
   - The `#[tauri::command]` adapter spawns a named thread that calls the helper and uses `AppHandle::emit` in the emitter closure.

2. In-memory cache:
   - Provide a tiny, global TTL cache module with Mutex-protected map keyed by string. Keep behaviour simple: `get_cached(key, ttl_secs) -> Option<Value>` and `set_cached(key, value)`.

3. DB cache:
   - Add a `steam_app_details(app_id TEXT PRIMARY KEY, details_json TEXT NOT NULL, fetched_at TEXT NOT NULL)` table and index on `fetched_at`.
   - Helpers: `find_cached_steam_app_details(connection, app_id, stale_before) -> Result<Option<Value>, String>` and `cache_steam_app_details(connection, app_id, details) -> Result<(), String>`.
   - Use `ON CONFLICT(app_id) DO UPDATE` semantics to upsert cache entries.

4. Caller integration:
   - For single-app callers that previously fetched `appdetails` directly, consult `find_cached_steam_app_details` first; if fresh, use cached payload; otherwise perform fetch and `cache_steam_app_details` best-effort.
   - For batched operations (library sync, metadata refresh) prefer batch endpoints; update DB caches in bulk when possible.

5. Tests:
   - Unit tests for cache helpers (in-memory and DB) using in-memory SQLite or temp DB path.
   - Tests for worker helpers that pass an injected emitter capturing results via `mpsc::channel` to validate both success and error cases.
   - Integration smoke tests (optional) that run `cargo test` followed by a small `tauri dev` smoke scenario during CI where feasible.

6. Frontend:
   - Register event listeners early (e.g., bootstrap `src/main.ts`) to avoid missed events.
   - Keep UI callers idempotent: starting a background scan multiple times should be safe.

## Migration Checklist

 - [ ] Identify all blocking command code paths and map them to worker + cache candidates.
 - [ ] Add in-memory TTL cache for local filesystem scans.
 - [ ] Add DB `steam_app_details` table and helpers.
 - [ ] Update single-app callers to consult DB cache and best-effort write-back.
 - [ ] Implement non-blocking Tauri commands that spawn workers and emit typed events.
 - [ ] Add unit tests for cache helpers and worker helpers.
 - [ ] Add integration tests that exercise worker helpers and validate emitted events.
 - [ ] Run full `cargo test` and `npm run build` and iterate on failures.

## Acceptance Criteria

 - Long-running local scans are moved off the UI command thread (e.g., `start_local_steam_scan` spawns a worker) and frontend receives `local-scan-complete` or `local-scan-error` events.
 - In-memory TTL cache prevents repeated filesystem scans within the configured TTL.
 - DB-backed `steam_app_details` cache exists and is consulted by single-app callers; cache writes are best-effort and do not break primary functionality.
 - Unit and integration tests for cache and worker helpers exist and pass under `cargo test`.
 - Frontend build completes and event listeners are registered without causing missed events.

## Consequences and Trade-offs

 - Positive: improved UI responsiveness, reduced external network load, and better testability.
 - Negative: increased implementation complexity, additional testing surface, and risk of cache staleness; these are mitigated by conservative TTLs and explicit event-driven refresh semantics.

## Rollback Plan

 - Because changes are incremental, rollbacks can be performed by reverting the small commits that introduce worker helpers, cache table schema, or caller wiring. Keep commits focused and isolated to facilitate reversion.

## References

- PLAN.md (migration roadmap)
- ADR 0001, ADR 0002, ADR 0003
- Implementation notes and test scaffolds in `src-tauri/src/interface/tauri/commands/library.rs` and `src-tauri/src/lib.rs` (cache helpers and test modules)
