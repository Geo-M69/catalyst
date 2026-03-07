---
adr: 0005
title: Phase 8 & 9 — Security Hardening and Contract Cleanup
status: accepted
date: 2026-03-06
---

Context
-------

During Phase 7 we moved blocking Steam and filesystem scans off the UI thread, added short-lived in-memory caching and a DB-backed app-details cache, and provided an event-driven background scanning pathway. Following that work we performed an initial security hardening pass (Phase 8) and a contract cleanup (Phase 9).

Goals
-----

- Reduce IPC attack surface and unnecessary Tauri capability exposure.
- Harden session persistence and content security policy (CSP).
- Remove unused or legacy command surface (local credentials) to reduce maintenance burden and dead code.
- Preserve developer ergonomics (dev-only CSP relaxations) while enforcing stricter production policies.

Decision
--------

1. Security hardening (Phase 8)
   - Persisted session tokens are written using restrictive Unix permissions (mode 0o600) when running on Unix platforms.
   - Production CSP tightened in `src-tauri/tauri.conf.json` to avoid unsafe sources. For local development the frontend injects a dev-only CSP meta tag (in `src/main.ts`) that relaxes `style-src` and font origins to make Vite-based development ergonomic; this meta injection is gated to `import.meta.env.DEV`.
   - Began capability/ACL audit: conservative recommendation to remove unused capabilities such as `core:tray` and `core:menu` where no native tray/menu APIs are used; retain `core:event` and `opener` which are actively used. Full capability pruning must be validated by regenerating ACL manifests and running the app through smoke tests.
   - Added a unit test asserting the session token file has restrictive permissions (Unix).

2. Contract cleanup (Phase 9)
   - Unregister and delete local-credential Tauri commands that are unused or deliberately deprecated: `register`, `login` and the frontend-facing `get_steam_status` wrapper.
   - Rationale: authentication in Catalyst is centered on Steam SSO (`start_steam_auth`). Local credential flows were unused by the UI and increase surface area and maintenance burden.
   - Implementation: removed IPC registration entries from the Tauri invoke handler, removed the Tauri command wrappers, and deleted the now-dead helper types and validation functions (`AuthUserRow`, `AuthResponse`, `SteamStatusResponse`, `find_auth_user_by_email`, `normalize_email`, `validate_password`, `is_email_like`). Tests were run to ensure no regressions (unit tests passed).

Consequences
------------

- Positive
  - Smaller attack surface: fewer commands exposed over IPC.
  - Fewer maintenance points: reduced dead/unused code and types.
  - More consistent auth story (single SSO pathway).

- Tradeoffs / Risks
  - Removing local credential helpers is irreversible in the short term (they are gone from the working tree). If local credentials are required later, re-implementation is necessary. To mitigate this, changes were made in a single commit and an ADR was recorded to document the decision and rationale.
  - Capability removal (ACL) must be done conservatively — removing a capability that is actually needed by a plugin (or by platform-specific code) can break features. Any capability pruning must be followed by a regenerate + test cycle.

Migration / Rollback
-------------------

- Rollback: revert the commit that removed the types/commands.
- Migration: if local credential support is re-introduced it should be implemented behind a feature flag and accompanied by tests and a clear security review.

Notes on testing and verification
---------------------------------

- `cargo test` was executed after removal; all unit tests passed.
- A dev smoke test was run (Vite dev + Tauri backend) to confirm the app starts with the relaxed dev CSP and that the background worker paths still emit events.

Follow-ups
----------

1. Regenerate capability manifests and prune capabilities conservatively (remove `core:tray` and `core:menu` if confirmed unused), then run full smoke tests across supported platforms.
2. Add a small regression test suite for command authorization flows (Phase 8 item).
3. Record the removal in the release notes and reference this ADR.

References
----------

- Code changes: IPC unregistering and deletion of helpers in `src-tauri/src/*` (auth and library paths).
- Dev CSP injection: `src/main.ts` (DEV-gated meta tag).
- Production CSP: `src-tauri/tauri.conf.json`.
