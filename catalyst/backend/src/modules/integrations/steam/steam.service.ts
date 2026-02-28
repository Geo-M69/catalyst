import { randomUUID } from "node:crypto";

import { env } from "../../../config/env.js";
import { HttpError } from "../../../shared/errors/http-error.js";
import { libraryService } from "../../library/library.service.js";
import { usersService } from "../../users/users.service.js";

import { steamClient } from "./steam.client.js";
import { mapSteamGameToLibraryGame } from "./steam.mapper.js";

interface PendingAuthState {
  userId: string;
  expiresAt: number;
}

interface SteamCallbackResult {
  userId: string;
  steamId: string;
  syncedGames: number;
}

const AUTH_STATE_TTL_MS = 10 * 60 * 1000;
const STEAM_CLAIMED_ID_PATTERN = /\/id\/(\d{17})$/;

class SteamService {
  private readonly pendingAuthStates = new Map<string, PendingAuthState>();

  public createAuthorization(userId: string): { authorizationUrl: string } {
    this.clearExpiredStates();

    const state = randomUUID();
    this.pendingAuthStates.set(state, {
      userId,
      expiresAt: Date.now() + AUTH_STATE_TTL_MS
    });

    const callbackUrl = new URL("/auth/steam/callback", env.APP_BASE_URL);
    callbackUrl.searchParams.set("state", state);

    return {
      authorizationUrl: steamClient.buildAuthorizationUrl(callbackUrl.toString(), env.APP_BASE_URL)
    };
  }

  public async completeAuthorization(params: URLSearchParams): Promise<SteamCallbackResult> {
    this.clearExpiredStates();

    const state = params.get("state");
    if (!state) {
      throw new HttpError(400, "Missing OAuth state", "VALIDATION_ERROR");
    }

    const pendingState = this.pendingAuthStates.get(state);
    if (!pendingState) {
      throw new HttpError(400, "Invalid or expired OAuth state", "VALIDATION_ERROR");
    }

    this.pendingAuthStates.delete(state);

    const isValidResponse = await steamClient.verifyOpenIdResponse(params);
    if (!isValidResponse) {
      throw new HttpError(401, "Steam login verification failed", "STEAM_AUTH_FAILED");
    }

    const claimedId = params.get("openid.claimed_id");
    if (!claimedId) {
      throw new HttpError(400, "Missing Steam claimed ID", "VALIDATION_ERROR");
    }

    const steamIdMatch = claimedId.match(STEAM_CLAIMED_ID_PATTERN);
    const steamId = steamIdMatch?.[1];
    if (!steamId) {
      throw new HttpError(400, "Invalid Steam claimed ID format", "VALIDATION_ERROR");
    }

    usersService.linkSteamAccount(pendingState.userId, steamId);
    const syncedGames = await this.syncOwnedGames(pendingState.userId);

    return {
      userId: pendingState.userId,
      steamId,
      syncedGames
    };
  }

  public async syncOwnedGames(userId: string): Promise<number> {
    const user = usersService.getOrCreateUser(userId);
    const steamId = user.integrations.steamId;
    if (!steamId) {
      throw new HttpError(400, "User is not linked to Steam", "STEAM_NOT_LINKED");
    }

    const ownedGames = await steamClient.getOwnedGames(steamId);
    const mappedGames = ownedGames.map(mapSteamGameToLibraryGame);

    libraryService.replaceProviderGames(userId, "steam", mappedGames);

    return mappedGames.length;
  }

  public getLinkStatus(userId: string): { linked: boolean; steamId?: string } {
    const user = usersService.getOrCreateUser(userId);
    return {
      linked: user.integrations.steamId !== undefined,
      steamId: user.integrations.steamId
    };
  }

  private clearExpiredStates(): void {
    const now = Date.now();
    for (const [state, value] of this.pendingAuthStates.entries()) {
      if (value.expiresAt <= now) {
        this.pendingAuthStates.delete(state);
      }
    }
  }
}

export const steamService = new SteamService();
