import { mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

import Database from "better-sqlite3";

import { env } from "../../config/env.js";
import { logger } from "../../config/logger.js";

type SqliteDatabase = InstanceType<typeof Database>;

let database: SqliteDatabase | null = null;

const runMigrations = (db: SqliteDatabase): void => {
  db.exec(`
    CREATE TABLE IF NOT EXISTS users (
      id TEXT PRIMARY KEY,
      email TEXT NOT NULL UNIQUE,
      password_hash TEXT NOT NULL,
      steam_id TEXT UNIQUE,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS sessions (
      token_hash TEXT PRIMARY KEY,
      user_id TEXT NOT NULL,
      created_at TEXT NOT NULL,
      expires_at TEXT NOT NULL,
      last_seen_at TEXT NOT NULL,
      FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
    CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);

    CREATE TABLE IF NOT EXISTS games (
      user_id TEXT NOT NULL,
      provider TEXT NOT NULL,
      external_id TEXT NOT NULL,
      name TEXT NOT NULL,
      playtime_minutes INTEGER NOT NULL,
      artwork_url TEXT,
      last_synced_at TEXT NOT NULL,
      PRIMARY KEY (user_id, provider, external_id),
      FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_games_user_id ON games(user_id);
    CREATE INDEX IF NOT EXISTS idx_games_provider ON games(provider);
  `);
};

export const initializeDatabase = (): SqliteDatabase => {
  if (database) {
    return database;
  }

  const dbPath = resolve(process.cwd(), env.DATABASE_PATH);
  mkdirSync(dirname(dbPath), { recursive: true });

  const db = new Database(dbPath);
  db.pragma("journal_mode = WAL");
  db.pragma("foreign_keys = ON");

  runMigrations(db);

  logger.info(`SQLite database initialized at ${dbPath}`);
  database = db;
  return db;
};

export const getDatabase = (): SqliteDatabase => {
  return database ?? initializeDatabase();
};
