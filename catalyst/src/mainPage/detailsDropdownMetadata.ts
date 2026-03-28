import type { GameStoreMetadata } from "./storeMetadata";
import type { GameResponse } from "./types";

interface LegacyGameFields {
  description?: string;
  release_date?: string;
}

export interface DetailsDropdownSnapshot {
  description: string;
  developersText: string;
  franchiseText: string;
  headerImage?: string;
  primaryPublisher?: string;
  publishersText: string;
  releaseDateText: string;
}

const toLegacyGame = (game: GameResponse): GameResponse & LegacyGameFields => {
  return game as GameResponse & LegacyGameFields;
};

const firstNonEmpty = (...values: Array<string | null | undefined>): string | undefined => {
  for (const value of values) {
    const trimmed = value?.trim();
    if (trimmed) {
      return trimmed;
    }
  }
  return undefined;
};

const joinOrDash = (values?: string[]): string => {
  if (!Array.isArray(values) || values.length === 0) {
    return "-";
  }
  return values.map((value) => value.trim()).filter(Boolean).join(", ") || "-";
};

const inferFranchiseFromTitle = (name: string): string | undefined => {
  const title = name.trim();
  if (!title) {
    return undefined;
  }
  for (const separator of [":", " - ", " — "]) {
    if (title.includes(separator)) {
      return title.split(separator)[0]?.trim() || undefined;
    }
  }
  return undefined;
};

export const buildDetailsDropdownSnapshot = (game: GameResponse): DetailsDropdownSnapshot => {
  const legacy = toLegacyGame(game);
  const publishers = Array.isArray(game.publishers) ? game.publishers : undefined;
  const primaryPublisher = firstNonEmpty(publishers?.[0]);
  const headerImage = firstNonEmpty(legacy.headerImage, game.headerImage);
  const snapshot: DetailsDropdownSnapshot = {
    description: firstNonEmpty(legacy.shortDescription, game.shortDescription, legacy.description) ?? "",
    developersText: joinOrDash(game.developers),
    publishersText: joinOrDash(game.publishers),
    franchiseText: firstNonEmpty(game.franchise) ?? "-",
    releaseDateText: firstNonEmpty(legacy.release_date, game.releaseDate) ?? "-",
  };

  if (headerImage) {
    snapshot.headerImage = headerImage;
  }
  if (primaryPublisher) {
    snapshot.primaryPublisher = primaryPublisher;
  }

  return snapshot;
};

export const resolveFranchiseLabel = (
  game: GameResponse,
  metadata: GameStoreMetadata,
  fallbackPublisher?: string
): string => {
  const explicitFranchise = firstNonEmpty(metadata.franchise ?? undefined);
  if (explicitFranchise) {
    return explicitFranchise;
  }

  const inferredFromTitle = inferFranchiseFromTitle(game.name);
  if (inferredFromTitle) {
    return inferredFromTitle;
  }

  const publisher = firstNonEmpty(metadata.publishers?.[0], fallbackPublisher);
  return publisher ?? "-";
};
