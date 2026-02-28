# Catalyst Backend

Express + TypeScript backend for account auth, Steam linking, and game-library sync.

## Endpoints

- `GET /health`
- `POST /auth/register`
- `POST /auth/login`
- `POST /auth/logout`
- `GET /auth/session`
- `GET /auth/steam/start`
- `GET /auth/steam/callback`
- `POST /integrations/steam/sync`
- `GET /integrations/steam/status`
- `GET /library`

## Quick Start

```bash
cd backend
npm install
cp .env.example .env
npm run dev
```

## Auth and Session Flow

1. Frontend calls `POST /auth/register` or `POST /auth/login` with JSON credentials.
2. Backend sets an HTTP-only session cookie.
3. Frontend sends future requests with credentials enabled (`credentials: "include"`).
4. Protected routes (`/auth/session`, `/library`, `/integrations/steam/*`, `/auth/steam/start`) require that cookie.

## Steam Link Flow

1. Authenticated frontend calls `GET /auth/steam/start`.
2. Frontend opens returned `authorizationUrl` in browser.
3. Steam redirects to `/auth/steam/callback`.
4. Backend verifies OpenID response, links `steamId`, syncs owned games, and redirects to:
   `FRONTEND_BASE_URL + FRONTEND_STEAM_CALLBACK_PATH` with status query params.

## Storage and Security

- Uses SQLite for users, sessions, and games.
- Uses HTTP-only cookie sessions (session token hash stored in DB).
- Rate limiting is enabled on auth and Steam endpoints.
- Steam API key stays server-side.

## Production Notes

- Set `SESSION_COOKIE_SECURE=true` in production.
- Set `TRUST_PROXY=true` if behind a reverse proxy/load balancer.
- SQLite is fine for early stages; consider Postgres when scaling beyond a single backend instance.
