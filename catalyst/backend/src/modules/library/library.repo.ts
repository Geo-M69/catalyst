import { getDatabase } from "../../shared/db/database.js";

import type { Game, GameProvider } from "./library.types.js";

interface GameRow {
  user_id: string;
  provider: string;
  external_id: string;
  name: string;
  playtime_minutes: number;
  artwork_url: string | null;
  last_synced_at: string;
}

const mapRowToGame = (row: GameRow): Game => ({
  id: `${row.provider}:${row.external_id}`,
  provider: row.provider as GameProvider,
  externalId: row.external_id,
  name: row.name,
  playtimeMinutes: row.playtime_minutes,
  artworkUrl: row.artwork_url ?? undefined,
  lastSyncedAt: row.last_synced_at
});

class LibraryRepository {
  public listByUser(userId: string): Game[] {
    const db = getDatabase();

    const rows = db
      .prepare(
        `
        SELECT user_id, provider, external_id, name, playtime_minutes, artwork_url, last_synced_at
        FROM games
        WHERE user_id = ?
        ORDER BY name COLLATE NOCASE ASC
      `
      )
      .all(userId) as GameRow[];

    return rows.map(mapRowToGame);
  }

  public replaceProviderGames(userId: string, provider: GameProvider, games: Game[]): void {
    const db = getDatabase();

    const transaction = db.transaction((currentUserId: string, currentProvider: GameProvider, nextGames: Game[]) => {
      db.prepare(
        `
        DELETE FROM games
        WHERE user_id = ? AND provider = ?
      `
      ).run(currentUserId, currentProvider);

      const insertStatement = db.prepare(
        `
        INSERT INTO games (user_id, provider, external_id, name, playtime_minutes, artwork_url, last_synced_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
      `
      );

      for (const game of nextGames) {
        insertStatement.run(
          currentUserId,
          currentProvider,
          game.externalId,
          game.name,
          game.playtimeMinutes,
          game.artworkUrl ?? null,
          game.lastSyncedAt
        );
      }
    });

    transaction(userId, provider, games);
  }
}

export const libraryRepository = new LibraryRepository();
