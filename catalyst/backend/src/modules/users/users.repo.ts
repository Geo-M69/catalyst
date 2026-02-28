import type { User } from "./users.types.js";

class UsersRepository {
  private readonly users = new Map<string, User>();

  public getOrCreate(userId: string): User {
    const existingUser = this.users.get(userId);
    if (existingUser) {
      return existingUser;
    }

    const timestamp = new Date().toISOString();
    const newUser: User = {
      id: userId,
      integrations: {},
      createdAt: timestamp,
      updatedAt: timestamp
    };

    this.users.set(userId, newUser);
    return newUser;
  }

  public findById(userId: string): User | null {
    return this.users.get(userId) ?? null;
  }

  public setSteamId(userId: string, steamId: string): User {
    const user = this.getOrCreate(userId);
    const updatedUser: User = {
      ...user,
      integrations: {
        ...user.integrations,
        steamId
      },
      updatedAt: new Date().toISOString()
    };

    this.users.set(userId, updatedUser);
    return updatedUser;
  }
}

export const usersRepository = new UsersRepository();
