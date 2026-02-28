import { createHash, randomBytes } from "node:crypto";

import bcrypt from "bcryptjs";
import type { CookieOptions, Response } from "express";

import { env } from "../../config/env.js";
import { HttpError } from "../../shared/errors/http-error.js";
import { usersService } from "../users/users.service.js";
import type { User } from "../users/users.types.js";

import { sessionRepository } from "./session.repo.js";
import type { SessionAuthResult } from "./session.types.js";

const SESSION_TOKEN_BYTES = 32;

const hashSessionToken = (sessionToken: string): string => {
  return createHash("sha256").update(sessionToken).digest("hex");
};

const buildNormalizedEmail = (email: string): string => {
  return email.trim().toLowerCase();
};

class SessionService {
  public async register(email: string, password: string): Promise<SessionAuthResult> {
    const normalizedEmail = buildNormalizedEmail(email);

    const existingUser = usersService.getAuthUserByEmail(normalizedEmail);
    if (existingUser) {
      throw new HttpError(409, "Email is already in use", "EMAIL_TAKEN");
    }

    const passwordHash = await bcrypt.hash(password, 12);
    const authRecord = usersService.createUser(normalizedEmail, passwordHash);
    const sessionToken = this.createSession(authRecord.user.id);

    return {
      user: authRecord.user,
      sessionToken
    };
  }

  public async login(email: string, password: string): Promise<SessionAuthResult> {
    const normalizedEmail = buildNormalizedEmail(email);
    const authRecord = usersService.getAuthUserByEmail(normalizedEmail);

    if (!authRecord) {
      throw new HttpError(401, "Invalid email or password", "INVALID_CREDENTIALS");
    }

    const isPasswordValid = await bcrypt.compare(password, authRecord.passwordHash);
    if (!isPasswordValid) {
      throw new HttpError(401, "Invalid email or password", "INVALID_CREDENTIALS");
    }

    const sessionToken = this.createSession(authRecord.user.id);
    return {
      user: authRecord.user,
      sessionToken
    };
  }

  public getUserBySessionToken(sessionToken: string): User | null {
    const nowIso = new Date().toISOString();
    const tokenHash = hashSessionToken(sessionToken);
    const session = sessionRepository.findValidByTokenHash(tokenHash, nowIso);

    if (!session) {
      return null;
    }

    const authUser = usersService.getAuthUserById(session.userId);
    if (!authUser) {
      sessionRepository.deleteByTokenHash(tokenHash);
      return null;
    }

    sessionRepository.touchSession(tokenHash, nowIso);
    return authUser.user;
  }

  public invalidateSession(sessionToken: string): void {
    const tokenHash = hashSessionToken(sessionToken);
    sessionRepository.deleteByTokenHash(tokenHash);
  }

  public createSessionForUser(userId: string): string {
    usersService.getRequiredUser(userId);
    return this.createSession(userId);
  }

  public cleanupExpiredSessions(): void {
    const nowIso = new Date().toISOString();
    sessionRepository.deleteExpiredSessions(nowIso);
  }

  public setSessionCookie(response: Response, sessionToken: string): void {
    response.cookie(env.SESSION_COOKIE_NAME, sessionToken, this.getSessionCookieOptions());
  }

  public clearSessionCookie(response: Response): void {
    response.clearCookie(env.SESSION_COOKIE_NAME, {
      ...this.getSessionCookieOptions(),
      maxAge: undefined,
      expires: new Date(0)
    });
  }

  private createSession(userId: string): string {
    const now = new Date();
    const expiresAt = new Date(now.getTime() + this.getSessionTtlMs());

    const sessionToken = randomBytes(SESSION_TOKEN_BYTES).toString("base64url");
    const tokenHash = hashSessionToken(sessionToken);

    sessionRepository.create({
      tokenHash,
      userId,
      createdAt: now.toISOString(),
      expiresAt: expiresAt.toISOString(),
      lastSeenAt: now.toISOString()
    });

    return sessionToken;
  }

  private getSessionCookieOptions(): CookieOptions {
    return {
      httpOnly: true,
      sameSite: "lax",
      secure: env.SESSION_COOKIE_SECURE,
      maxAge: this.getSessionTtlMs(),
      path: "/"
    };
  }

  private getSessionTtlMs(): number {
    return env.SESSION_TTL_DAYS * 24 * 60 * 60 * 1000;
  }
}

export const sessionService = new SessionService();
