import { getDatabase } from "../../shared/db/database.js";

import type { SessionRecord } from "./session.types.js";

interface SessionRow {
  token_hash: string;
  user_id: string;
  created_at: string;
  expires_at: string;
  last_seen_at: string;
}

const mapRowToSession = (row: SessionRow): SessionRecord => ({
  tokenHash: row.token_hash,
  userId: row.user_id,
  createdAt: row.created_at,
  expiresAt: row.expires_at,
  lastSeenAt: row.last_seen_at
});

class SessionRepository {
  public create(session: SessionRecord): void {
    const db = getDatabase();

    db.prepare(
      `
      INSERT INTO sessions (token_hash, user_id, created_at, expires_at, last_seen_at)
      VALUES (?, ?, ?, ?, ?)
    `
    ).run(session.tokenHash, session.userId, session.createdAt, session.expiresAt, session.lastSeenAt);
  }

  public findValidByTokenHash(tokenHash: string, nowIso: string): SessionRecord | null {
    const db = getDatabase();

    const row = db
      .prepare(
        `
        SELECT token_hash, user_id, created_at, expires_at, last_seen_at
        FROM sessions
        WHERE token_hash = ?
          AND expires_at > ?
      `
      )
      .get(tokenHash, nowIso) as SessionRow | undefined;

    if (!row) {
      return null;
    }

    return mapRowToSession(row);
  }

  public touchSession(tokenHash: string, lastSeenAt: string): void {
    const db = getDatabase();

    db.prepare(
      `
      UPDATE sessions
      SET last_seen_at = ?
      WHERE token_hash = ?
    `
    ).run(lastSeenAt, tokenHash);
  }

  public deleteByTokenHash(tokenHash: string): void {
    const db = getDatabase();

    db.prepare(
      `
      DELETE FROM sessions
      WHERE token_hash = ?
    `
    ).run(tokenHash);
  }

  public deleteExpiredSessions(nowIso: string): void {
    const db = getDatabase();

    db.prepare(
      `
      DELETE FROM sessions
      WHERE expires_at <= ?
    `
    ).run(nowIso);
  }
}

export const sessionRepository = new SessionRepository();
