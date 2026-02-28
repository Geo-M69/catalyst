import { randomUUID } from "node:crypto";

import { getDatabase } from "../../shared/db/database.js";

import type { AuthUserRecord, User } from "./users.types.js";

interface UserRow {
  id: string;
  email: string;
  password_hash: string;
  steam_id: string | null;
  created_at: string;
  updated_at: string;
}

const mapRowToUser = (row: UserRow): User => ({
  id: row.id,
  email: row.email,
  integrations: {
    steamId: row.steam_id ?? undefined
  },
  createdAt: row.created_at,
  updatedAt: row.updated_at
});

const mapRowToAuthRecord = (row: UserRow): AuthUserRecord => ({
  user: mapRowToUser(row),
  passwordHash: row.password_hash
});

class UsersRepository {
  public create(email: string, passwordHash: string): AuthUserRecord {
    const db = getDatabase();

    const id = randomUUID();
    const timestamp = new Date().toISOString();

    db.prepare(
      `
      INSERT INTO users (id, email, password_hash, steam_id, created_at, updated_at)
      VALUES (?, ?, ?, NULL, ?, ?)
    `
    ).run(id, email, passwordHash, timestamp, timestamp);

    const insertedRow = db
      .prepare(
        `
        SELECT id, email, password_hash, steam_id, created_at, updated_at
        FROM users
        WHERE id = ?
      `
      )
      .get(id) as UserRow | undefined;

    if (!insertedRow) {
      throw new Error("Failed to create user");
    }

    return mapRowToAuthRecord(insertedRow);
  }

  public findById(userId: string): User | null {
    const db = getDatabase();

    const row = db
      .prepare(
        `
        SELECT id, email, password_hash, steam_id, created_at, updated_at
        FROM users
        WHERE id = ?
      `
      )
      .get(userId) as UserRow | undefined;

    if (!row) {
      return null;
    }

    return mapRowToUser(row);
  }

  public findAuthById(userId: string): AuthUserRecord | null {
    const db = getDatabase();

    const row = db
      .prepare(
        `
        SELECT id, email, password_hash, steam_id, created_at, updated_at
        FROM users
        WHERE id = ?
      `
      )
      .get(userId) as UserRow | undefined;

    if (!row) {
      return null;
    }

    return mapRowToAuthRecord(row);
  }

  public findAuthByEmail(email: string): AuthUserRecord | null {
    const db = getDatabase();

    const row = db
      .prepare(
        `
        SELECT id, email, password_hash, steam_id, created_at, updated_at
        FROM users
        WHERE email = ?
      `
      )
      .get(email) as UserRow | undefined;

    if (!row) {
      return null;
    }

    return mapRowToAuthRecord(row);
  }

  public setSteamId(userId: string, steamId: string): User | null {
    const db = getDatabase();

    const updatedAt = new Date().toISOString();

    const result = db
      .prepare(
        `
        UPDATE users
        SET steam_id = ?, updated_at = ?
        WHERE id = ?
      `
      )
      .run(steamId, updatedAt, userId);

    if (result.changes === 0) {
      return null;
    }

    return this.findById(userId);
  }
}

export const usersRepository = new UsersRepository();
