import { openUrl } from "@tauri-apps/plugin-opener";
import type { GameResponse } from "./types";

export const openSteamConnectedUrl = async (
  steamProtocolUrl: string,
  webFallbackUrl: string
): Promise<void> => {
  try {
    await openUrl(steamProtocolUrl);
    return;
  } catch {
    await openUrl(webFallbackUrl);
  }
};

export const collectSteamTagSuggestions = (games: GameResponse[]): string[] => {
  const tagsByKey = new Map<string, string>();

  for (const game of games) {
    for (const rawTag of game.steamTags ?? []) {
      const tag = rawTag.trim();
      if (tag.length === 0) {
        continue;
      }
      const key = tag.toLocaleLowerCase();
      if (!tagsByKey.has(key)) {
        tagsByKey.set(key, tag);
      }
    }
  }

  return [...tagsByKey.values()].sort((left, right) =>
    left.localeCompare(right, undefined, { sensitivity: "base" })
  );
};

export const formatLibraryRefreshAgeLabel = (elapsedMs: number): string => {
  const elapsedSeconds = Math.floor(elapsedMs / 1000);
  if (elapsedSeconds < 15) {
    return "Synced just now";
  }

  if (elapsedSeconds < 60) {
    return `Synced ${elapsedSeconds}s ago`;
  }

  const elapsedMinutes = Math.floor(elapsedSeconds / 60);
  if (elapsedMinutes < 60) {
    return `Synced ${elapsedMinutes}m ago`;
  }

  const elapsedHours = Math.floor(elapsedMinutes / 60);
  if (elapsedHours < 24) {
    return `Synced ${elapsedHours}h ago`;
  }

  const elapsedDays = Math.floor(elapsedHours / 24);
  return `Synced ${elapsedDays}d ago`;
};
