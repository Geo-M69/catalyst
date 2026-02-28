import { env } from "../../../config/env.js";
import { HttpError } from "../../../shared/errors/http-error.js";

import type { SteamOwnedGame, SteamOwnedGamesApiResponse } from "./steam.types.js";

const STEAM_OPENID_ENDPOINT = "https://steamcommunity.com/openid/login";
const STEAM_WEB_API_ENDPOINT = "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/";

class SteamClient {
  public buildAuthorizationUrl(returnTo: string, realm: string): string {
    const url = new URL(STEAM_OPENID_ENDPOINT);

    url.searchParams.set("openid.ns", "http://specs.openid.net/auth/2.0");
    url.searchParams.set("openid.mode", "checkid_setup");
    url.searchParams.set("openid.return_to", returnTo);
    url.searchParams.set("openid.realm", realm);
    url.searchParams.set("openid.identity", "http://specs.openid.net/auth/2.0/identifier_select");
    url.searchParams.set("openid.claimed_id", "http://specs.openid.net/auth/2.0/identifier_select");

    return url.toString();
  }

  public async verifyOpenIdResponse(params: URLSearchParams): Promise<boolean> {
    const verificationParams = new URLSearchParams(params);
    verificationParams.set("openid.mode", "check_authentication");

    const response = await fetch(STEAM_OPENID_ENDPOINT, {
      method: "POST",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded"
      },
      body: verificationParams.toString()
    });

    if (!response.ok) {
      throw new HttpError(502, "Steam OpenID verification failed", "STEAM_OPENID_ERROR");
    }

    const body = await response.text();
    return body.includes("is_valid:true");
  }

  public async getOwnedGames(steamId: string): Promise<SteamOwnedGame[]> {
    if (env.STEAM_API_KEY.trim().length === 0) {
      throw new HttpError(500, "Missing STEAM_API_KEY configuration", "STEAM_API_KEY_MISSING");
    }

    const url = new URL(STEAM_WEB_API_ENDPOINT);
    url.searchParams.set("key", env.STEAM_API_KEY);
    url.searchParams.set("steamid", steamId);
    url.searchParams.set("include_appinfo", "true");
    url.searchParams.set(
      "include_played_free_games",
      env.STEAM_SYNC_INCLUDE_PLAYED_FREE_GAMES ? "true" : "false"
    );
    url.searchParams.set("format", "json");

    const response = await fetch(url, {
      method: "GET"
    });

    if (!response.ok) {
      throw new HttpError(502, "Steam GetOwnedGames request failed", "STEAM_API_ERROR");
    }

    const body = (await response.json()) as SteamOwnedGamesApiResponse;
    return body.response?.games ?? [];
  }
}

export const steamClient = new SteamClient();
