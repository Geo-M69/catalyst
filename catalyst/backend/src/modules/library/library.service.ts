import { usersService } from "../users/users.service.js";

import { libraryRepository } from "./library.repo.js";
import type { Game, GameProvider } from "./library.types.js";

class LibraryService {
  public listUserGames(userId: string): Game[] {
    usersService.getRequiredUser(userId);
    return libraryRepository.listByUser(userId);
  }

  public replaceProviderGames(userId: string, provider: GameProvider, games: Game[]): void {
    usersService.getRequiredUser(userId);
    libraryRepository.replaceProviderGames(userId, provider, games);
  }
}

export const libraryService = new LibraryService();
