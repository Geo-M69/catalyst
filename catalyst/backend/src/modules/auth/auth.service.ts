import { env } from "../../config/env.js";

import { steamService } from "../integrations/steam/steam.service.js";

interface CallbackPayload {
  status: "success" | "error";
  userId?: string;
  steamId?: string;
  syncedGames?: number;
  message?: string;
}

class AuthService {
  public startSteamAuth(userId: string): { authorizationUrl: string } {
    return steamService.createAuthorization(userId);
  }

  public async completeSteamAuth(params: URLSearchParams): Promise<{
    userId: string;
    steamId: string;
    syncedGames: number;
  }> {
    return steamService.completeAuthorization(params);
  }

  public buildFrontendCallbackUrl(payload: CallbackPayload): string {
    const callbackUrl = new URL(env.FRONTEND_STEAM_CALLBACK_PATH, env.FRONTEND_BASE_URL);
    callbackUrl.searchParams.set("status", payload.status);

    if (payload.userId) {
      callbackUrl.searchParams.set("userId", payload.userId);
    }

    if (payload.steamId) {
      callbackUrl.searchParams.set("steamId", payload.steamId);
    }

    if (payload.syncedGames !== undefined) {
      callbackUrl.searchParams.set("syncedGames", String(payload.syncedGames));
    }

    if (payload.message) {
      callbackUrl.searchParams.set("message", payload.message);
    }

    return callbackUrl.toString();
  }
}

export const authService = new AuthService();
