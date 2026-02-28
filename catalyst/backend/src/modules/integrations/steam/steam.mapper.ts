import type { Game } from "../../library/library.types.js";

import type { SteamOwnedGame } from "./steam.types.js";

const buildArtworkUrl = (appid: number, logoHash?: string): string | undefined => {
  if (!logoHash) {
    return undefined;
  }

  return `https://media.steampowered.com/steamcommunity/public/images/apps/${appid}/${logoHash}.jpg`;
};

export const mapSteamGameToLibraryGame = (steamGame: SteamOwnedGame): Game => {
  const externalId = String(steamGame.appid);

  return {
    id: `steam:${externalId}`,
    provider: "steam",
    externalId,
    name: steamGame.name?.trim() || `Steam App ${externalId}`,
    playtimeMinutes: steamGame.playtime_forever ?? 0,
    artworkUrl: buildArtworkUrl(steamGame.appid, steamGame.img_logo_url),
    lastSyncedAt: new Date().toISOString()
  };
};
