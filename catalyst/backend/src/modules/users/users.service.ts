import { HttpError } from "../../shared/errors/http-error.js";

import { usersRepository } from "./users.repo.js";
import type { AuthUserRecord, User } from "./users.types.js";

interface SqliteError extends Error {
  code?: string;
}

const isUniqueConstraintError = (error: unknown): error is SqliteError => {
  return (
    error instanceof Error &&
    typeof (error as SqliteError).code === "string" &&
    (error as SqliteError).code === "SQLITE_CONSTRAINT_UNIQUE"
  );
};

class UsersService {
  public createUser(email: string, passwordHash: string): AuthUserRecord {
    try {
      return usersRepository.create(email, passwordHash);
    } catch (error) {
      if (isUniqueConstraintError(error)) {
        throw new HttpError(409, "Email is already in use", "EMAIL_TAKEN");
      }

      throw error;
    }
  }

  public getUser(userId: string): User | null {
    return usersRepository.findById(userId);
  }

  public getRequiredUser(userId: string): User {
    const user = this.getUser(userId);
    if (!user) {
      throw new HttpError(404, "User not found", "USER_NOT_FOUND");
    }

    return user;
  }

  public getAuthUserById(userId: string): AuthUserRecord | null {
    return usersRepository.findAuthById(userId);
  }

  public getAuthUserByEmail(email: string): AuthUserRecord | null {
    return usersRepository.findAuthByEmail(email);
  }

  public linkSteamAccount(userId: string, steamId: string): User {
    try {
      const updatedUser = usersRepository.setSteamId(userId, steamId);
      if (!updatedUser) {
        throw new HttpError(404, "User not found", "USER_NOT_FOUND");
      }

      return updatedUser;
    } catch (error) {
      if (error instanceof HttpError) {
        throw error;
      }

      if (isUniqueConstraintError(error)) {
        throw new HttpError(409, "Steam account is already linked to another user", "STEAM_ALREADY_LINKED");
      }

      throw error;
    }
  }
}

export const usersService = new UsersService();
