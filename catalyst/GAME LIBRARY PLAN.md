### Phased Plan: Game Details View + Custom Hero Background

**Summary**
- Deliver the Steam-style game details experience in **5 phases** so each milestone is shippable and testable.
- Keep details as an in-place view inside `main.html`, opened from game-card click/`Enter`.
- Use the **custom background image from the existing customization flow** as the hero background at the top of the details page, with clear fallback behavior.

**Phased Implementation**
1. **Phase 1: Navigation and View-State Foundation**
- Add app-level view state (`library` vs `game-details`) and selected game tracking.
- Wire single-click and keyboard `Enter` on `.game-card` to open details.
- Add back navigation (`Back` control + `history.pushState/popstate`) and preserve library scroll/filter state.
- Hide left sidebar in details mode and expand details to full content area.

2. **Phase 2: Details Page Skeleton and Action Bar**
- Build the details layout shell (hero header, top stats/action row, main content columns).
- Add primary actions: Play/Install, Settings, Favorite toggle, Show Game Details.
- Reuse existing game context menu actions from the new Settings button (no duplicate action system).

3. **Phase 3: Hero Background from Customization Artwork (Steam-like)**
- Reuse existing customization artwork retrieval (`getGameCustomizationArtwork`) when opening details.
- Hero background source order: `customizationArtwork.background` -> Steam background candidates -> neutral gradient fallback.
- Add non-blocking image loading with graceful fallback to avoid layout shift.
- Refresh hero background after Properties/custom-artwork changes so updated art appears immediately when returning to details.

4. **Phase 4: Content Sections and Placeholder Strategy**
- Implement sections: timeline/activity, friends activity, achievements, notes, screenshots, review, trading cards.
- Populate with real data where currently available; use explicit empty/coming-soon placeholders elsewhere.
- Keep Notes/Review read-only placeholder cards in v1 (no edit persistence yet).

5. **Phase 5: Backend Data Completion and Hardening**
- Extend Steam owned-games mapping to persist `lastPlayedAt` (from `rtime_last_played`) in DB/API.
- Add idempotent DB migration for nullable `games.last_played_at` and include it in library queries.
- Final polish pass for keyboard accessibility, responsive behavior, and error/empty states.

**Public Interfaces / Type Changes**
- Extend backend `GameResponse` payload population for `lastPlayedAt` (frontend type already supports it).
- Extend context-menu integration surface with `showGameDetails(game)` and programmatic open for the details settings button.
- No new artwork IPC command required; reuse existing customization-artwork contract.

**Test Plan**
- Opening details: single click and `Enter` from game cards.
- Navigation: back button/history returns to prior library state.
- Settings button in details opens the same context menu and actions as library.
- Hero artwork behavior:
- Uses customization background when present.
- Falls back correctly when custom background is missing or invalid.
- Updates after custom-artwork changes in Properties flow.
- Backend:
- Migration adds `last_played_at` safely and idempotently.
- Library response includes `lastPlayedAt` when available.
- Validation commands: `npm run build`, `cargo check --manifest-path src-tauri/Cargo.toml`.

**Assumptions**
- Phase count is fixed to **5 phases** for this implementation plan.
- Details view remains in the existing authenticated entrypoint (`main.html`), not a new page.
- Left sidebar is hidden while details are open.
- Timeline/friends/achievements/review/cards/screenshots remain placeholder-first where backend data is unavailable.
