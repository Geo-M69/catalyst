# Catalyst Backend

Express + TypeScript backend scaffold for account linking and game-library sync.

## Endpoints

- `GET /health`
- `GET /auth/steam/start?userId=<id>`
- `GET /auth/steam/callback`
- `POST /integrations/steam/sync`
- `GET /integrations/steam/status?userId=<id>`
- `GET /library?userId=<id>`

## Quick Start

```bash
cd backend
npm install
cp .env.example .env
npm run dev
```

## Frontend Flow

1. Frontend calls `GET /auth/steam/start?userId=<internalUserId>`.
2. Frontend opens `authorizationUrl` in browser.
3. Steam sends user back to `/auth/steam/callback`.
4. Backend verifies OpenID response, links `steamId`, syncs owned games, and redirects to:
   `FRONTEND_BASE_URL + FRONTEND_STEAM_CALLBACK_PATH` with query params.

## Notes

- Steam API key must stay server-side.
- `GetOwnedGames` depends on profile privacy.
- Re-sync can be triggered through `POST /integrations/steam/sync`.
