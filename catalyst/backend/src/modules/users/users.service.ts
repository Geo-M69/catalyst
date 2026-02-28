import { usersRepository } from "./users.repo.js";
import type { User } from "./users.types.js";

class UsersService {
  public getOrCreateUser(userId: string): User {
    return usersRepository.getOrCreate(userId);
  }

  public getUser(userId: string): User | null {
    return usersRepository.findById(userId);
  }

  public linkSteamAccount(userId: string, steamId: string): User {
    return usersRepository.setSteamId(userId, steamId);
  }
}

export const usersService = new UsersService();
