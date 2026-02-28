import type { Game, GameProvider } from "./library.types.js";

class LibraryRepository {
  private readonly gamesByUser = new Map<string, Map<string, Game>>();

  public listByUser(userId: string): Game[] {
    return Array.from(this.gamesByUser.get(userId)?.values() ?? []);
  }

  public replaceProviderGames(userId: string, provider: GameProvider, games: Game[]): void {
    const existingGames = this.gamesByUser.get(userId) ?? new Map<string, Game>();

    for (const existingGame of existingGames.values()) {
      if (existingGame.provider === provider) {
        existingGames.delete(existingGame.id);
      }
    }

    for (const game of games) {
      existingGames.set(game.id, game);
    }

    this.gamesByUser.set(userId, existingGames);
  }
}

export const libraryRepository = new LibraryRepository();
