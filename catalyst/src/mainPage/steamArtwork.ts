import type { GameResponse } from "./types";

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

const addUniqueCandidate = (value: string | undefined, seen: Set<string>, candidates: string[]): void => {
  const trimmed = value?.trim();
  if (!trimmed || seen.has(trimmed)) {
    return;
  }

  seen.add(trimmed);
  candidates.push(trimmed);
};

const addSteamArtworkCandidates = (
  appId: string,
  filenames: readonly string[],
  seen: Set<string>,
  candidates: string[]
): void => {
  for (const baseUrl of STEAM_APP_CDN_BASE_URLS) {
    for (const filename of filenames) {
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
  const normalizedProvider = game.provider.trim().toLowerCase();
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
      ], seen, candidates);
    }
  }

  addUniqueCandidate(game.artworkUrl, seen, candidates);

  if (kind === "wide-cover") {
    addUniqueCandidate(
      normalizedProvider === "steam" && /^\d+$/.test(normalizedExternalId)
        ? `https://cdn.cloudflare.steamstatic.com/steam/apps/${normalizedExternalId}/capsule_467x181.jpg`
        : undefined,
      seen,
      candidates
    );
  }

  return candidates;
};
