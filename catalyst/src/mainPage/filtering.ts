import type { GameResponse, LibraryFilters } from "./types";

const normalize = (value: string): string => value.trim().toLowerCase();

const parseDate = (isoDate: string): number => {
  const value = Date.parse(isoDate);
  return Number.isNaN(value) ? 0 : value;
};

const getSourceFromProvider = (provider: string): "steam" | "epic-games" | "other" => {
  const normalized = normalize(provider);
  if (normalized.includes("steam")) {
    return "steam";
  }
  if (normalized.includes("epic")) {
    return "epic-games";
  }
  return "other";
};

const hasTag = (values: string[] | undefined, expected: string): boolean => {
  if (!values || values.length === 0) {
    return true;
  }
  return values.some((value) => normalize(value) === expected);
};

const hasExactTag = (values: string[] | undefined, expected: string): boolean => {
  if (expected.length === 0) {
    return true;
  }
  if (!values || values.length === 0) {
    return false;
  }
  return values.some((value) => normalize(value) === expected);
};

const includesTagText = (values: string[] | undefined, expected: string): boolean => {
  if (expected.length === 0) {
    return true;
  }
  if (!values || values.length === 0) {
    return false;
  }
  return values.some((value) => normalize(value).includes(expected));
};

export const applyLibraryFilters = (
  games: GameResponse[],
  filters: LibraryFilters
): GameResponse[] => {
  const searchTerm = normalize(filters.search);
  const steamTagFilter = normalize(filters.steamTag);
  const collectionFilter = normalize(filters.collection);
  const now = Date.now();
  const thirtyDaysAgo = now - (30 * 24 * 60 * 60 * 1000);

  const filtered = games.filter((game) => {
    const source = getSourceFromProvider(game.provider);
    const lastPlayedAt = game.lastPlayedAt ? parseDate(game.lastPlayedAt) : 0;
    const fallbackRecentlyPlayed = game.playtimeMinutes > 0 && parseDate(game.lastSyncedAt) >= thirtyDaysAgo;
    const recentlyPlayed = lastPlayedAt > 0 ? lastPlayedAt >= thirtyDaysAgo : fallbackRecentlyPlayed;

    if (
      searchTerm.length > 0
      && !normalize(game.name).includes(searchTerm)
      && !normalize(game.provider).includes(searchTerm)
      && !normalize(game.kind).includes(searchTerm)
      && !includesTagText(game.steamTags, searchTerm)
      && !includesTagText(game.collections, searchTerm)
    ) {
      return false;
    }

    if (filters.filterBy === "installed") {
      const installed = typeof game.installed === "boolean" ? game.installed : game.playtimeMinutes > 0;
      if (!installed) {
        return false;
      }
    }

    if (filters.filterBy === "not-installed") {
      const installed = typeof game.installed === "boolean" ? game.installed : game.playtimeMinutes > 0;
      if (installed) {
        return false;
      }
    }

    if (filters.filterBy === "favorites" && !game.favorite) {
      return false;
    }

    if (filters.filterBy === "recently-played" && !recentlyPlayed) {
      return false;
    }

    if (filters.filterBy === "never-played" && game.playtimeMinutes > 0) {
      return false;
    }

    if (filters.platform !== "all" && !hasTag(game.platforms, filters.platform)) {
      return false;
    }

    if (filters.source !== "all" && source !== filters.source) {
      return false;
    }

    if (steamTagFilter.length > 0 && (source !== "steam" || !hasExactTag(game.steamTags, steamTagFilter))) {
      return false;
    }

    if (collectionFilter.length > 0 && !hasExactTag(game.collections, collectionFilter)) {
      return false;
    }

    if (filters.kind !== "all" && game.kind !== filters.kind) {
      return false;
    }

    if (filters.genre !== "all" && !hasTag(game.genres, filters.genre)) {
      return false;
    }

    return true;
  });

  return [...filtered].sort((left, right) => {
    if (filters.sortBy === "most-played") {
      return right.playtimeMinutes - left.playtimeMinutes;
    }

    if (filters.sortBy === "least-played") {
      return left.playtimeMinutes - right.playtimeMinutes;
    }

    if (filters.sortBy === "alphabetical-reverse") {
      return right.name.localeCompare(left.name, undefined, { sensitivity: "base" });
    }

    return left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
  });
};
