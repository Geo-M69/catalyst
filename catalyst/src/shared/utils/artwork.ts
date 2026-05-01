import type { GameResponse } from "../models/library";

export type SteamLibraryArtworkKind = "cover" | "background" | "logo" | "wide-cover";

const STEAM_APP_CDN_BASE_URLS = [
  "https://cdn.cloudflare.steamstatic.com/steam/apps",
  "https://cdn.akamai.steamstatic.com/steam/apps",
] as const;

const isSteamAppGame = (game: GameResponse): boolean => {
  const provider = game.provider.trim().toLowerCase();
  const externalId = game.externalId.trim();
  return provider === "steam" && /^\d+$/.test(externalId);
};

export const addUniqueCandidate = (
  candidate: string | undefined,
  seen: Set<string>,
  candidates: string[]
): void => {
  const trimmed = candidate?.trim();
  if (!trimmed || seen.has(trimmed)) {
    return;
  }

  seen.add(trimmed);
  candidates.push(trimmed);
};

export const addCandidates = (values: string[], seen: Set<string>, candidates: string[]): void => {
  for (const value of values) {
    addUniqueCandidate(value, seen, candidates);
  }
};

const addSteamArtworkCandidates = (
  appId: string,
  filenames: readonly string[],
  seen: Set<string>,
  candidates: string[]
): void => {
  // Interleave CDN hosts per filename so callers can fail over quickly when one
  // edge host is slow/unreachable for a specific user/network.
  for (const filename of filenames) {
    for (const baseUrl of STEAM_APP_CDN_BASE_URLS) {
      addUniqueCandidate(`${baseUrl}/${appId}/${filename}`, seen, candidates);
    }
  }
};

export const getSteamArtworkCandidates = (
  game: GameResponse,
  kind: SteamLibraryArtworkKind
): string[] => {
  const candidates: string[] = [];
  const seen = new Set<string>();
  const normalizedExternalId = game.externalId.trim();

  if (isSteamAppGame(game)) {
    const appId = normalizedExternalId;
    if (kind === "cover") {
      addSteamArtworkCandidates(appId, [
        "library_600x900_2x.jpg",
        "library_600x900.jpg",
      ], seen, candidates);
    } else if (kind === "background") {
      addSteamArtworkCandidates(appId, [
        "library_hero.jpg",
        "library_hero_blur.jpg",
      ], seen, candidates);
    } else if (kind === "logo") {
      addSteamArtworkCandidates(appId, [
        "logo.png",
        "library_logo.png",
      ], seen, candidates);
    } else if (kind === "wide-cover") {
      addSteamArtworkCandidates(appId, [
        "library_capsule.jpg",
        "capsule_616x353.jpg",
        "header.jpg",
        "capsule_467x181.jpg",
      ], seen, candidates);
    }
  }

  // Prefer larger metadata artwork before legacy logo/icon-derived fields.
  addUniqueCandidate(game.headerImage, seen, candidates);
  addUniqueCandidate(game.artworkUrl, seen, candidates);

  return candidates;
};
